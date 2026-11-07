use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::atomic;
use crate::declarations::DECLARATIONS_FILE;
use crate::git::{Git, TreeChange};
use crate::item::{ItemId, Items, KeyPath, keyed_file};
use crate::paths::{self, Paths};
use crate::setup::Backup;
use crate::{references, secret};

const CATALOG_FILES: [&str; 2] = [".claude/CLAUDE.md", ".claude/keybindings.json"];
const CATALOG_DIRS: [&str; 7] = [
    ".claude/rules",
    ".claude/skills",
    ".claude/agents",
    ".claude/commands",
    ".claude/output-styles",
    ".claude/workflows",
    ".claude/themes",
];
const SKIPPED_DIRS: [&str; 2] = [".claude/skills/synced", ".claude/skills/.trash"];
const SKIPPED_NAMES: [&str; 1] = [".DS_Store"];
const CREDENTIALS_FILE: &str = ".claude/.credentials.json";

pub(crate) fn load_local(
    git: &Git,
    paths: &Paths,
    json: &Items,
    include: &[KeyPath],
) -> Result<Items> {
    let mut files = Vec::new();
    for file in CATALOG_FILES {
        if is_regular_file(&paths.local(file))? {
            files.push(file.to_owned());
        }
    }
    for dir in CATALOG_DIRS {
        walk(paths, dir, &mut files)?;
    }
    for included in include.iter().filter(|path| is_file_include(path)) {
        if is_regular_file(&paths.local(&included.file))? {
            files.push(included.file.clone());
        } else {
            walk(paths, &included.file, &mut files)?;
        }
    }
    let referenced = references::find(paths, json, &files);
    files.extend(referenced);
    files.sort();
    files.dedup();
    let absolute: Vec<PathBuf> = files.iter().map(|file| paths.local(file)).collect();
    let blobs = git.hash_files(&absolute)?;
    Ok(files
        .iter()
        .zip(blobs)
        .zip(&absolute)
        .map(|((file, blob), local)| (ItemId::whole_file(file), value(&blob, is_executable(local))))
        .collect())
}

fn is_file_include(path: &KeyPath) -> bool {
    path.path.is_empty() && keyed_file(&path.file).is_none() && path.file != CREDENTIALS_FILE
}

fn walk(paths: &Paths, relative: &str, found: &mut Vec<String>) -> Result<()> {
    if SKIPPED_DIRS.contains(&relative) {
        return Ok(());
    }
    let dir = paths.local(relative);
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
            walk(paths, &path, found)?;
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

pub(crate) fn load_snapshot(git: &Git, commit: &str, include: &[KeyPath]) -> Result<Items> {
    Ok(git
        .list_files(commit)?
        .into_iter()
        .filter(|entry| is_snapshot_file(&entry.path, include))
        .map(|entry| {
            (
                ItemId::whole_file(&entry.path),
                value(&entry.blob, entry.executable),
            )
        })
        .collect())
}

fn is_snapshot_file(file: &str, include: &[KeyPath]) -> bool {
    let synced = paths::is_harness_path(file)
        || include
            .iter()
            .any(|path| is_file_include(path) && path.covers(file, &[]));
    synced && keyed_file(file).is_none() && file != CREDENTIALS_FILE && file != DECLARATIONS_FILE
}

pub(crate) fn changes(outgoing: &Items, remote: &Items, paths: &Paths) -> Result<Vec<TreeChange>> {
    let mut changes = Vec::new();
    for (id, value) in outgoing.iter().filter(|(id, _)| id.is_whole_file()) {
        if remote.get(id) == Some(value) {
            continue;
        }
        let file = paths.local(&id.file);
        let content = fs::read(&file).with_context(|| format!("cannot read {}", file.display()))?;
        changes.push(TreeChange::Write {
            path: id.file.clone(),
            content,
            executable: is_marked_executable(value),
        });
    }
    for id in remote
        .keys()
        .filter(|id| id.is_whole_file() && !outgoing.contains_key(id))
    {
        changes.push(TreeChange::Delete {
            path: id.file.clone(),
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
            let text = std::str::from_utf8(content).ok()?;
            secret::looks_like_token(text).then(|| ItemId::whole_file(path).key_path())
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
        let file = paths.local(&id.file);
        match fs::read(&file) {
            Ok(old) => backup.save(Path::new(&id.file), &old)?,
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
    for (id, value) in local.iter_mut().filter(|(id, _)| id.is_whole_file()) {
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
