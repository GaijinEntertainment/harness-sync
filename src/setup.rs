use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::declarations::Declarations;
use crate::git::{Git, TreeChange};
use crate::item::{Items, JSON_SOURCES, JsonFile, KeyPath, Source, json_document, json_items};
use crate::json_edit::{self, Edit};
use crate::paths::Paths;
use crate::{atomic, files};

#[derive(Debug)]
pub(crate) struct Backup {
    dir: PathBuf,
}

impl Backup {
    pub(crate) fn new(paths: &Paths) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        Self {
            dir: paths.backups_dir().join(stamp.to_string()),
        }
    }

    pub(crate) fn save(&self, relative: &Path, content: &[u8]) -> Result<()> {
        let path = self.dir.join(relative);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        fs::write(&path, content)
            .with_context(|| format!("cannot write the backup {}", path.display()))
    }
}

pub(crate) fn load_local(paths: &Paths, git: &Git, declarations: &Declarations) -> Result<Items> {
    let mut items = Items::new();
    for source in JSON_SOURCES {
        let path = paths.json_file(source);
        match fs::read(&path) {
            Ok(bytes) => items.extend(parse(source, &bytes, &path, &declarations.include)?),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
        }
    }
    items.retain(|id, _| !declarations.excludes(id));
    let files = files::load_local(git, paths, &items, &declarations.include)?;
    items.extend(files);
    items.retain(|id, _| !declarations.excludes(id));
    Ok(items)
}

pub(crate) fn load_snapshot(
    git: &Git,
    commit: Option<&str>,
    declarations: &Declarations,
) -> Result<Items> {
    let mut items = load_whole_snapshot(git, commit, &declarations.include)?;
    items.retain(|id, _| !declarations.excludes(id));
    Ok(items)
}

fn load_whole_snapshot(git: &Git, commit: Option<&str>, include: &[KeyPath]) -> Result<Items> {
    let Some(commit) = commit else {
        return Ok(Items::new());
    };
    let specs: Vec<String> = JSON_SOURCES
        .iter()
        .map(|source| format!("{commit}:{}", source.snapshot_file()))
        .collect();
    let mut items = files::load_snapshot(git, commit)?;
    for ((source, spec), content) in JSON_SOURCES.iter().zip(&specs).zip(git.read_files(&specs)?) {
        if let Some(bytes) = content {
            items.extend(parse(*source, &bytes, Path::new(spec), include)?);
        }
    }
    Ok(items)
}

fn parse(file: JsonFile, bytes: &[u8], origin: &Path, include: &[KeyPath]) -> Result<Items> {
    let document = serde_json::from_slice(bytes)
        .with_context(|| format!("{} is not valid JSON", origin.display()))?;
    json_items(file, document, include)
        .with_context(|| format!("cannot read the items of {}", origin.display()))
}

pub(crate) fn json_changes(items: &Items) -> Result<Vec<TreeChange>> {
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
            Ok(TreeChange::Write {
                path: source.snapshot_file().to_owned(),
                content,
                executable: false,
            })
        })
        .collect()
}

pub(crate) fn excluded_removals(
    git: &Git,
    commit: Option<&str>,
    declarations: &Declarations,
) -> Result<Vec<TreeChange>> {
    let (excluded, kept): (Items, Items) = load_whole_snapshot(git, commit, &declarations.include)?
        .into_iter()
        .partition(|(id, _)| declarations.excludes(id));
    let mut changes = if excluded.keys().any(|id| id.source != Source::File) {
        json_changes(&kept)?
    } else {
        Vec::new()
    };
    changes.extend(
        excluded
            .keys()
            .filter(|id| id.source == Source::File)
            .map(|id| TreeChange::Delete {
                path: files::snapshot_path(id),
            }),
    );
    Ok(changes)
}

pub(crate) fn write_local(
    paths: &Paths,
    backup: &Backup,
    file: JsonFile,
    edits: &[Edit<'_>],
) -> Result<()> {
    let path = paths.json_file(file);
    let original = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    let edited = json_edit::apply(original.as_deref().unwrap_or("{}\n"), edits)
        .with_context(|| format!("cannot edit {}", path.display()))?;
    if let Some(original) = &original {
        let name = path
            .file_name()
            .context("a Claude Code file has no file name")?;
        backup.save(Path::new(name), original.as_bytes())?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    atomic::write(&path, edited.as_bytes())
}
