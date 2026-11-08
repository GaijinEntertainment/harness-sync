use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::declarations::Declarations;
use crate::harness::{KeyedFile, Selection, keyed_files};
use crate::item::{Items, KeyPath, json_document, json_items};
use crate::json_edit::{self, Edit};
use crate::paths::Paths;
use crate::storage::{FileChange, Storage};
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

pub(crate) fn load_local(
    paths: &Paths,
    storage: &dyn Storage,
    declarations: &Declarations,
    selection: &Selection,
) -> Result<Items> {
    let mut items = Items::new();
    for keyed in selection
        .harnesses()
        .flat_map(|harness| harness.keyed_files)
        .filter(|keyed| !paths.is_behind_symlink(keyed.path))
    {
        let path = paths.local(keyed.path);
        match fs::read(&path) {
            Ok(bytes) => items.extend(parse(keyed, &bytes, &path, &declarations.include)?),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
        }
    }
    items.retain(|id, _| !declarations.excludes(id));
    let catalog = files::catalog(paths, &items, &declarations.include, selection)?;
    items.extend(files::load_local(storage, paths, &catalog)?);
    items.retain(|id, _| !declarations.excludes(id));
    Ok(items)
}

pub(crate) fn load_snapshot(
    storage: &dyn Storage,
    commit: Option<&str>,
    declarations: &Declarations,
    selection: &Selection,
) -> Result<Items> {
    let mut items = load_whole_snapshot(storage, commit, &declarations.include)?;
    items.retain(|id, _| selection.syncs(&id.file) && !declarations.excludes(id));
    Ok(items)
}

fn load_whole_snapshot(
    storage: &dyn Storage,
    commit: Option<&str>,
    include: &[KeyPath],
) -> Result<Items> {
    let Some(commit) = commit else {
        return Ok(Items::new());
    };
    let paths: Vec<&str> = keyed_files().map(|keyed| keyed.path).collect();
    let mut items = files::load_snapshot(storage, commit, include)?;
    for (keyed, content) in keyed_files().zip(storage.read_files(commit, &paths)?) {
        if let Some(bytes) = content {
            let origin = format!("{commit}:{}", keyed.path);
            items.extend(parse(keyed, &bytes, Path::new(&origin), include)?);
        }
    }
    Ok(items)
}

fn parse(keyed: &KeyedFile, bytes: &[u8], origin: &Path, include: &[KeyPath]) -> Result<Items> {
    let document = serde_json::from_slice(bytes)
        .with_context(|| format!("{} is not valid JSON", origin.display()))?;
    json_items(keyed, document, include)
        .with_context(|| format!("cannot read the items of {}", origin.display()))
}

pub(crate) fn json_changes(items: &Items, remote: &Items) -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();
    for keyed in keyed_files() {
        let holds_items = |items: &Items| items.keys().any(|id| id.file == keyed.path);
        if holds_items(items) {
            let mut content = serde_json::to_vec_pretty(&json_document(items, keyed.path))
                .with_context(|| {
                    format!("cannot serialize {} of the remote snapshot", keyed.path)
                })?;
            content.push(b'\n');
            changes.push(FileChange::Write {
                path: keyed.path.to_owned(),
                content,
                executable: false,
            });
        } else if holds_items(remote) {
            changes.push(FileChange::Delete {
                path: keyed.path.to_owned(),
            });
        }
    }
    Ok(changes)
}

pub(crate) fn excluded_removals(
    storage: &dyn Storage,
    commit: Option<&str>,
    declarations: &Declarations,
) -> Result<Vec<FileChange>> {
    let whole = load_whole_snapshot(storage, commit, &declarations.include)?;
    let (excluded, kept): (Items, Items) = whole
        .clone()
        .into_iter()
        .partition(|(id, _)| declarations.excludes(id));
    let mut changes = if excluded.keys().any(|id| !id.is_whole_file()) {
        json_changes(&kept, &whole)?
    } else {
        Vec::new()
    };
    changes.extend(
        excluded
            .keys()
            .filter(|id| id.is_whole_file())
            .map(|id| FileChange::Delete {
                path: id.file.clone(),
            }),
    );
    Ok(changes)
}

pub(crate) fn write_local(
    paths: &Paths,
    backup: &Backup,
    file: &str,
    edits: &[Edit<'_>],
) -> Result<()> {
    paths.ensure_no_symlink(file)?;
    let path = paths.local(file);
    let original = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    let edited = json_edit::apply(original.as_deref().unwrap_or("{}\n"), edits)
        .with_context(|| format!("cannot edit {}", path.display()))?;
    if let Some(original) = &original {
        backup.save(Path::new(file), original.as_bytes())?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    atomic::write(&path, edited.as_bytes())
}
