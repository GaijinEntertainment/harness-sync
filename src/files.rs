use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::atomic;
use crate::git::{Git, TreeChange};
use crate::item::{ItemId, Items, KeyPath, Source};
use crate::paths::Paths;
use crate::setup::Backup;
use crate::{references, secret};

const SNAPSHOT_DIR: &str = "files";
const CATALOG_FILES: [&str; 2] = ["CLAUDE.md", "keybindings.json"];
const CATALOG_DIRS: [&str; 7] = [
    "rules",
    "skills",
    "agents",
    "commands",
    "output-styles",
    "workflows",
    "themes",
];
const SKIPPED_DIRS: [&str; 1] = ["skills/synced"];
const SKIPPED_NAMES: [&str; 1] = [".DS_Store"];

pub(crate) fn load_local(git: &Git, paths: &Paths, json: &Items) -> Result<Items> {
    let claude_dir = &paths.claude_dir;
    let mut relative = Vec::new();
    for name in CATALOG_FILES {
        if is_regular_file(&claude_dir.join(name))? {
            relative.push(name.to_owned());
        }
    }
    for dir in CATALOG_DIRS {
        walk(claude_dir, dir, &mut relative)?;
    }
    let referenced = references::find(paths, json, &relative);
    relative.extend(referenced);
    let absolute: Vec<PathBuf> = relative.iter().map(|path| claude_dir.join(path)).collect();
    let blobs = git.hash_files(&absolute)?;
    Ok(relative
        .iter()
        .zip(blobs)
        .zip(&absolute)
        .map(|((path, blob), file)| (item_id(path), value(&blob, is_executable(file))))
        .collect())
}

fn walk(claude_dir: &Path, relative: &str, found: &mut Vec<String>) -> Result<()> {
    if SKIPPED_DIRS.contains(&relative) {
        return Ok(());
    }
    let dir = claude_dir.join(relative);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", dir.display())),
    };
    let mut entries = entries
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("cannot read {}", dir.display()))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if SKIPPED_NAMES.contains(&name.as_str()) || name.contains('\n') {
            continue;
        }
        let path = format!("{relative}/{name}");
        let kind = entry
            .file_type()
            .with_context(|| format!("cannot read {}", entry.path().display()))?;
        if kind.is_dir() {
            walk(claude_dir, &path, found)?;
        } else if kind.is_file() {
            found.push(path);
        }
    }
    Ok(())
}

fn is_regular_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("cannot read {}", path.display())),
    }
}

pub(crate) fn load_snapshot(git: &Git, commit: &str) -> Result<Items> {
    Ok(git
        .list_files(commit, SNAPSHOT_DIR)?
        .into_iter()
        .filter_map(|entry| {
            let relative = entry.path.strip_prefix(SNAPSHOT_DIR)?.strip_prefix('/')?;
            Some((item_id(relative), value(&entry.blob, entry.executable)))
        })
        .collect())
}

pub(crate) fn changes(
    outgoing: &Items,
    remote: &Items,
    claude_dir: &Path,
) -> Result<Vec<TreeChange>> {
    let mut changes = Vec::new();
    for (id, value) in outgoing.iter().filter(|(id, _)| id.source == Source::File) {
        if remote.get(id) == Some(value) {
            continue;
        }
        let file = claude_dir.join(relative(id));
        let content = fs::read(&file).with_context(|| format!("cannot read {}", file.display()))?;
        changes.push(TreeChange::Write {
            path: snapshot_path(id),
            content,
            executable: is_marked_executable(value),
        });
    }
    for id in remote
        .keys()
        .filter(|id| id.source == Source::File && !outgoing.contains_key(id))
    {
        changes.push(TreeChange::Delete {
            path: snapshot_path(id),
        });
    }
    Ok(changes)
}

pub(crate) fn token_like(changes: &[TreeChange]) -> Vec<KeyPath> {
    changes
        .iter()
        .filter_map(|change| {
            let TreeChange::Write { path, content, .. } = change else {
                return None;
            };
            let relative = path.strip_prefix(SNAPSHOT_DIR)?.strip_prefix('/')?;
            let text = std::str::from_utf8(content).ok()?;
            secret::looks_like_token(text).then(|| item_id(relative).key_path())
        })
        .collect()
}

pub(crate) fn write_local(
    paths: &Paths,
    git: &Git,
    backup: &Backup,
    writes: &[(&ItemId, Option<&Value>)],
) -> Result<()> {
    let blobs: Vec<String> = writes
        .iter()
        .filter_map(|(_, value)| {
            value
                .and_then(|value| value.get("blob"))
                .and_then(Value::as_str)
        })
        .map(str::to_owned)
        .collect();
    let mut contents = git.read_files(&blobs)?.into_iter();
    for (id, value) in writes {
        let relative = relative(id);
        let file = paths.claude_dir.join(&relative);
        match fs::read(&file) {
            Ok(old) => backup.save(&Path::new(SNAPSHOT_DIR).join(&relative), &old)?,
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err).with_context(|| format!("cannot read {}", file.display())),
        }
        let Some(value) = value else {
            fs::remove_file(&file).with_context(|| format!("cannot delete {}", file.display()))?;
            continue;
        };
        let content = contents
            .next()
            .flatten()
            .with_context(|| format!("the local repo has no content for {id}"))?;
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        atomic::write(&file, &content)?;
        set_executable(&file, is_marked_executable(value))?;
    }
    Ok(())
}

pub(crate) fn carry_executable(local: &mut Items, known: &[&Items]) {
    for (id, value) in local.iter_mut().filter(|(id, _)| id.source == Source::File) {
        let executable = known
            .iter()
            .find_map(|items| items.get(id))
            .is_some_and(is_marked_executable);
        if let Value::Object(fields) = value {
            fields.remove("executable");
            if executable {
                fields.insert("executable".to_owned(), Value::Bool(true));
            }
        }
    }
}

fn item_id(relative: &str) -> ItemId {
    ItemId {
        source: Source::File,
        path: relative.split('/').map(str::to_owned).collect(),
    }
}

fn relative(id: &ItemId) -> String {
    id.path.join("/")
}

fn snapshot_path(id: &ItemId) -> String {
    format!("{SNAPSHOT_DIR}/{}", relative(id))
}

fn value(blob: &str, executable: bool) -> Value {
    if executable {
        json!({"blob": blob, "executable": true})
    } else {
        json!({"blob": blob})
    }
}

fn is_marked_executable(value: &Value) -> bool {
    value.get("executable") == Some(&Value::Bool(true))
}

#[cfg(unix)]
fn is_executable(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(file).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_file: &Path) -> bool {
    false
}

#[cfg(unix)]
fn set_executable(file: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(file, fs::Permissions::from_mode(mode))
        .with_context(|| format!("cannot set the mode of {}", file.display()))
}

#[cfg(not(unix))]
fn set_executable(_file: &Path, _executable: bool) -> Result<()> {
    Ok(())
}
