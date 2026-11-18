use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::atomic;
use crate::declarations::DECLARATIONS_FILE;
use crate::harness::{self, Selection};
use crate::item::{ItemId, Items, KeyPath, is_home_path, is_within_ignoring_case};
use crate::paths::{Paths, STATE_DIR};
use crate::setup::Backup;
use crate::storage::{FileChange, Storage};
use crate::{references, secret};

const SKIPPED_NAMES: [&str; 2] = [".DS_Store", "node_modules"];

pub(crate) fn catalog(
    paths: &Paths,
    json: &Items,
    include: &[KeyPath],
    selection: &Selection,
) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for harness in selection.harnesses() {
        for &file in harness.files {
            if is_regular_file(&paths.local(file))? && !paths.is_behind_symlink(file) {
                files.push(file.to_owned());
            }
        }
        for dir in harness
            .dirs
            .iter()
            .filter(|dir| !paths.is_behind_symlink(dir))
        {
            walk(paths, dir, &mut files)?;
        }
    }
    let included = include.iter().filter(|path| {
        is_file_include(path) && selection.syncs(&path.file) && !paths.is_behind_symlink(&path.file)
    });
    for included in included {
        if is_regular_file(&paths.local(&included.file))? {
            files.push(included.file.clone());
        } else {
            walk(paths, &included.file, &mut files)?;
        }
    }
    let referenced = references::find(paths, json, &files);
    files.extend(referenced.into_iter().filter(|file| selection.syncs(file)));
    files.sort();
    files.dedup();
    Ok(files)
}

pub(crate) fn load_local(storage: &dyn Storage, paths: &Paths, files: &[String]) -> Result<Items> {
    let absolute: Vec<PathBuf> = files.iter().map(|file| paths.local(file)).collect();
    let blobs = storage.content_ids(&absolute)?;
    Ok(files
        .iter()
        .zip(blobs)
        .zip(&absolute)
        .map(|((file, blob), local)| (ItemId::whole_file(file), value(&blob, is_executable(local))))
        .collect())
}

pub(crate) fn can_sync_whole(file: &str) -> bool {
    let is_skipped_name = |name: &str| {
        SKIPPED_NAMES
            .iter()
            .any(|skipped| name.eq_ignore_ascii_case(skipped))
    };
    is_home_path(file)
        && !file.eq_ignore_ascii_case(DECLARATIONS_FILE)
        && !is_within_ignoring_case(file, STATE_DIR)
        && !harness::lists_as_never_whole(file)
        && !file.split('/').any(is_skipped_name)
}

fn is_file_include(path: &KeyPath) -> bool {
    path.path.is_empty() && can_sync_whole(&path.file)
}

fn walk(paths: &Paths, relative: &str, found: &mut Vec<String>) -> Result<()> {
    if !can_sync_whole(relative) {
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
        let path = format!("{relative}/{name}");
        let kind = entry
            .file_type()
            .with_context(|| format!("cannot read {}", entry.path().display()))?;
        if kind.is_dir() {
            walk(paths, &path, found)?;
        } else if kind.is_file() && can_sync_whole(&path) {
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

pub(crate) fn load_snapshot(
    storage: &dyn Storage,
    commit: &str,
    include: &[KeyPath],
) -> Result<Items> {
    Ok(storage
        .list_files(commit)?
        .into_iter()
        .filter(|file| is_snapshot_file(&file.path, include))
        .map(|file| {
            (
                ItemId::whole_file(&file.path),
                value(&file.content_id, file.executable),
            )
        })
        .collect())
}

fn is_snapshot_file(file: &str, include: &[KeyPath]) -> bool {
    let synced = harness::owner(file).is_some()
        || include
            .iter()
            .any(|path| is_file_include(path) && path.covers(file, &[]));
    synced && can_sync_whole(file)
}

pub(crate) fn unmanaged(
    storage: &dyn Storage,
    commit: Option<&str>,
    include: &[KeyPath],
) -> Result<Vec<String>> {
    let Some(commit) = commit else {
        return Ok(Vec::new());
    };
    Ok(storage
        .list_files(commit)?
        .into_iter()
        .map(|file| file.path)
        .filter(|file| !is_managed(file, include))
        .collect())
}

fn is_managed(file: &str, include: &[KeyPath]) -> bool {
    file == DECLARATIONS_FILE
        || harness::keyed_file(file).is_some()
        || is_snapshot_file(file, include)
}

pub(crate) fn changes(outgoing: &Items, remote: &Items, paths: &Paths) -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();
    for (id, value) in outgoing.iter().filter(|(id, _)| id.is_whole_file()) {
        if remote.get(id) == Some(value) {
            continue;
        }
        let file = paths.local(&id.file);
        let content = fs::read(&file).with_context(|| format!("cannot read {}", file.display()))?;
        changes.push(FileChange::Write {
            path: id.file.clone(),
            content,
            executable: is_marked_executable(value),
        });
    }
    for id in remote
        .keys()
        .filter(|id| id.is_whole_file() && !outgoing.contains_key(id))
    {
        changes.push(FileChange::Delete {
            path: id.file.clone(),
        });
    }
    Ok(changes)
}

pub(crate) fn token_like(changes: &[FileChange]) -> Vec<KeyPath> {
    changes
        .iter()
        .filter_map(|change| {
            let FileChange::Write { path, content, .. } = change else {
                return None;
            };
            let text = std::str::from_utf8(content).ok()?;
            secret::looks_like_token(text).then(|| ItemId::whole_file(path).key_path())
        })
        .collect()
}

pub(crate) fn write_local(
    paths: &Paths,
    storage: &dyn Storage,
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
    let mut contents = storage.read_contents(&blobs)?.into_iter();
    for (id, value) in writes {
        paths.ensure_no_symlink(&id.file)?;
        let file = paths.local(&id.file);
        match fs::read(&file) {
            Ok(old) => backup.save(Path::new(&id.file), &old)?,
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err).with_context(|| format!("cannot read {}", file.display())),
        }
        if value.is_none() {
            fs::remove_file(&file).with_context(|| format!("cannot delete {}", file.display()))?;
            continue;
        }
        let content = contents
            .next()
            .flatten()
            .with_context(|| format!("the local repo has no content for {id}"))?;
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        atomic::write(&file, &content)?;
        #[cfg(unix)]
        set_executable(&file, value.is_some_and(is_marked_executable))?;
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
const fn is_executable(_file: &Path) -> bool {
    false
}

#[cfg(unix)]
fn set_executable(file: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(file, fs::Permissions::from_mode(mode))
        .with_context(|| format!("cannot set the mode of {}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_syncs_whole_only_outside_the_places_that_never_sync() {
        for file in [".claude/rules/a.md", ".agents/skills/x/SKILL.md"] {
            assert!(can_sync_whole(file), "{file}");
        }
        for file in [
            ".claude/settings.json",
            ".claude/.credentials.json",
            ".harness-sync/config.toml",
            "harness-sync.toml",
            ".claude/skills/synced/x/SKILL.md",
            "tools/node_modules/dep/index.js",
            ".claude/rules/.DS_Store",
            ".claude/rules/a:b.md",
            ".claude/rules/a\\b.md",
            ".claude/rules/a\rb.md",
            "C:/x",
            ".claude/../x",
            ".HARNESS-SYNC/config.toml",
            "Harness-Sync.toml",
            ".claude/.CREDENTIALS.json",
            ".claude/Settings.json",
            ".claude/skills/Synced/x/SKILL.md",
            "tools/NODE_MODULES/dep/index.js",
            ".claude/rules/.ds_store",
        ] {
            assert!(!can_sync_whole(file), "{file}");
        }
    }

    #[test]
    fn a_snapshot_file_is_managed_when_a_harness_or_the_repo_lists_hold_it() {
        let include = [".agents/skills".parse::<KeyPath>().unwrap()];

        for file in [
            "harness-sync.toml",
            ".claude/settings.json",
            ".claude.json",
            ".claude/rules/a.md",
            ".pi/agent/models.json",
            ".agents/skills/x/SKILL.md",
        ] {
            assert!(is_managed(file, &include), "{file}");
        }
        for file in [
            "README.md",
            "settings.json",
            "files/CLAUDE.md",
            ".agents/other.md",
            ".claude/.credentials.json",
            ".claude/rules/node_modules/dep/index.js",
        ] {
            assert!(!is_managed(file, &include), "{file}");
        }
    }
}
