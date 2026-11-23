//! Retired items: the items that the repo's `exclude` list took out of the sync while this machine still holds them,
//! and the `retired` commands that remove them or keep them.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::declarations::Declarations;
use crate::harness::Selection;
use crate::item::ItemId;
use crate::paths::Paths;
use crate::report::{Report, counted};
use crate::session::Session;
use crate::{atomic, pull, setup};

#[derive(Debug, Default, Serialize, Deserialize)]
struct RetiredFile {
    retired: Vec<String>,
}

/// Removes each retired item from the local setup, with a backup of each file that changes.
///
/// # Errors
///
/// Fails when this machine is not connected, the storage fails, or a harness file cannot be read or written.
pub fn remove(paths: &Paths) -> Result<Report> {
    let session = Session::open(paths)?;
    let retired = refresh(paths, &session, Vec::new())?;
    if !retired.is_empty() {
        let deletions: Vec<(&ItemId, Option<Value>)> =
            retired.iter().map(|id| (id, None)).collect();
        pull::write(paths, &session, &deletions)?;
        save(&paths.retired_items_file(), &BTreeSet::new())?;
    }
    let summary = format!(
        "removed {} from this machine",
        counted(retired.len(), "retired item")
    );
    Ok(report("retired remove", summary, &retired))
}

/// Keeps each retired item on this machine and stops the warning about it.
///
/// # Errors
///
/// Fails when the sync configuration or the list of retired items cannot be read, or the list cannot be written.
pub fn keep(paths: &Paths) -> Result<Report> {
    Config::load(&paths.config_file())?;
    let file = paths.retired_items_file();
    let retired = load(&file)?;
    if !retired.is_empty() {
        save(&file, &BTreeSet::new())?;
    }
    let summary = format!(
        "kept {} on this machine",
        counted(retired.len(), "retired item")
    );
    Ok(report("retired keep", summary, &retired))
}

fn report(command: &'static str, summary: String, retired: &BTreeSet<ItemId>) -> Report {
    Report {
        retired_items: retired.iter().map(ToString::to_string).collect(),
        ..Report::new(command, summary)
    }
}

/// The recorded retired items that this machine still holds and that the `exclude` list still covers.
pub(crate) fn recorded(paths: &Paths, session: &Session) -> Result<Vec<String>> {
    let retired = refresh(paths, session, Vec::new())?;
    Ok(retired.iter().map(ToString::to_string).collect())
}

/// Records the items that the pull of `session` takes out of the sync, and returns every retired item. A pull calls
/// it before it moves the base commit, which is what names those items.
pub(crate) fn record_pulled(paths: &Paths, session: &Session) -> Result<Vec<String>> {
    let retired = refresh(paths, session, excluded_since_base(session)?)?;
    Ok(retired.iter().map(ToString::to_string).collect())
}

fn refresh(paths: &Paths, session: &Session, added: Vec<ItemId>) -> Result<BTreeSet<ItemId>> {
    let file = paths.retired_items_file();
    let recorded = load(&file)?;
    let mut retired = recorded.clone();
    retired.extend(added);
    if !retired.is_empty() {
        let held = setup::load_local(
            paths,
            session.storage.as_ref(),
            &session.declarations.without_exclude(),
            &session.selection,
        )?;
        retired.retain(|id| session.declarations.excludes(id) && held.contains_key(id));
    }
    if retired != recorded {
        save(&file, &retired)?;
    }
    Ok(retired)
}

fn excluded_since_base(session: &Session) -> Result<Vec<ItemId>> {
    if session.base_commit == session.remote_commit {
        return Ok(Vec::new());
    }
    let storage = session.storage.as_ref();
    let base = session.base_commit.as_deref();
    let synced_at_base = setup::load_snapshot(
        storage,
        base,
        &Declarations::load(storage, base)?,
        &session.selection,
    )?;
    let without_base = Selection::new(&session.config.harnesses_without_base)?;
    Ok(synced_at_base
        .into_keys()
        .filter(|id| session.declarations.excludes(id) && !without_base.owns(&id.file))
        .collect())
}

fn load(path: &Path) -> Result<BTreeSet<ItemId>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    let file: RetiredFile =
        toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))?;
    file.retired
        .iter()
        .map(|id| id.parse())
        .collect::<Result<_>>()
        .with_context(|| format!("cannot parse {}", path.display()))
}

fn save(path: &Path, retired: &BTreeSet<ItemId>) -> Result<()> {
    if retired.is_empty() {
        return match fs::remove_file(path) {
            Err(err) if err.kind() != ErrorKind::NotFound => {
                Err(err).with_context(|| format!("cannot delete {}", path.display()))
            }
            _ => Ok(()),
        };
    }
    let file = RetiredFile {
        retired: retired.iter().map(ToString::to_string).collect(),
    };
    let text = toml::to_string(&file).context("cannot serialize the list of retired items")?;
    atomic::write(path, text.as_bytes())
}
