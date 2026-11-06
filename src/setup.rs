use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::atomic;
use crate::git::{Git, SnapshotFile};
use crate::item::{Items, JSON_SOURCES, Source, json_document, json_items};
use crate::json_edit::{self, Edit};
use crate::paths::Paths;

pub(crate) fn load_local(paths: &Paths) -> Result<Items> {
    let mut items = Items::new();
    for source in JSON_SOURCES {
        let path = paths.json_file(source);
        match fs::read(&path) {
            Ok(bytes) => items.extend(parse(source, &bytes, &path)?),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
        }
    }
    Ok(items)
}

pub(crate) fn load_snapshot(git: &Git, commit: Option<&str>) -> Result<Items> {
    let Some(commit) = commit else {
        return Ok(Items::new());
    };
    let specs: Vec<String> = JSON_SOURCES
        .iter()
        .map(|source| format!("{commit}:{}", source.snapshot_file()))
        .collect();
    let mut items = Items::new();
    for ((source, spec), content) in JSON_SOURCES.iter().zip(&specs).zip(git.read_files(&specs)?) {
        if let Some(bytes) = content {
            items.extend(parse(*source, &bytes, Path::new(spec))?);
        }
    }
    Ok(items)
}

fn parse(source: Source, bytes: &[u8], origin: &Path) -> Result<Items> {
    let document = serde_json::from_slice(bytes)
        .with_context(|| format!("{} is not valid JSON", origin.display()))?;
    json_items(source, document)
        .with_context(|| format!("cannot read the items of {}", origin.display()))
}

pub(crate) fn snapshot_files(items: &Items) -> Result<Vec<SnapshotFile>> {
    JSON_SOURCES
        .iter()
        .map(|&source| {
            let mut content = serde_json::to_vec_pretty(&json_document(items, source))
                .with_context(|| {
                    format!(
                        "cannot serialize {} of the remote snapshot",
                        source.snapshot_file()
                    )
                })?;
            content.push(b'\n');
            Ok(SnapshotFile {
                path: source.snapshot_file().to_owned(),
                content,
            })
        })
        .collect()
}

pub(crate) fn write_local(paths: &Paths, source: Source, edits: &[Edit<'_>]) -> Result<()> {
    let path = paths.json_file(source);
    let original = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    let edited = json_edit::apply(original.as_deref().unwrap_or("{}\n"), edits)
        .with_context(|| format!("cannot edit {}", path.display()))?;
    if let Some(original) = &original {
        backup(paths, &path, original.as_bytes())?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    atomic::write(&path, edited.as_bytes())
}

fn backup(paths: &Paths, file: &Path, content: &[u8]) -> Result<()> {
    let name = file
        .file_name()
        .context("a Claude Code file has no file name")?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let dir = paths.backups_dir().join(stamp.to_string());
    fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let path = dir.join(name);
    fs::write(&path, content).with_context(|| format!("cannot write the backup {}", path.display()))
}
