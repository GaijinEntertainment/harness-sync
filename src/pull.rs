use std::collections::BTreeSet;

use anyhow::{Result, ensure};
use serde_json::Value;

use crate::compare::{Difference, ItemState};
use crate::config::Config;
use crate::harness::keyed_files;
use crate::item::{ItemId, Items, KeyPath};
use crate::json_edit::Edit;
use crate::paths::Paths;
use crate::report::{ItemReport, Outcome, Report, with_missing_secrets};
use crate::session::Session;
use crate::setup::Backup;
use crate::{executables, files, home, secret, setup};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Local,
    Remote,
}

/// The questions that pull asks a person at a terminal.
pub(crate) trait Prompts {
    /// Asks which side wins `conflict`; `None` stops the questions about conflicts.
    fn ask_side(&self, conflict: &ItemReport) -> Result<Option<Side>>;

    /// Asks for the value of the secret at `key`; `None` leaves the secret missing.
    fn ask_secret(&self, key: &KeyPath) -> Result<Option<String>>;
}

#[derive(Debug, Clone)]
pub(crate) struct Take {
    pub(crate) id: ItemId,
    pub(crate) side: Side,
}

#[derive(Debug)]
pub(crate) struct Choices<'a> {
    pub(crate) prefer: Option<Side>,
    pub(crate) takes: &'a [Take],
}

impl Choices<'_> {
    fn side_for(&self, id: &ItemId) -> Option<Side> {
        self.takes
            .iter()
            .find(|take| &take.id == id)
            .map(|take| take.side)
            .or(self.prefer)
    }
}

pub(crate) fn run(
    paths: &Paths,
    choices: &Choices<'_>,
    prompts: Option<&dyn Prompts>,
) -> Result<Report> {
    let session = Session::open(paths)?;
    let differences = session.differences();
    let is_conflict = |id: &ItemId| {
        differences
            .iter()
            .any(|d| d.state == ItemState::Conflict && d.id == id)
    };
    let stray: Vec<String> = choices
        .takes
        .iter()
        .filter(|take| !is_conflict(&take.id))
        .map(|take| take.id.to_string())
        .collect();
    ensure!(
        stray.is_empty(),
        "--take names items that are not in conflict: {}",
        stray.join(", ")
    );
    let Some(remote_commit) = session.remote_commit.clone() else {
        return Ok(report(
            &session,
            &[],
            Outcome::Ok,
            "the remote branch is empty: nothing to pull".to_owned(),
        ));
    };
    let takes = match prompts {
        Some(prompts) => ask_sides(prompts, &differences, choices)?,
        None => choices.takes.to_vec(),
    };
    let choices = &Choices {
        prefer: choices.prefer,
        takes: &takes,
    };
    let unresolved = differences
        .iter()
        .filter(|d| d.state == ItemState::Conflict && choices.side_for(d.id).is_none())
        .count();
    if unresolved > 0 {
        let summary = format!(
            "{unresolved} conflicts, nothing written: choose a side with --take ITEM=local|remote or --prefer local|remote"
        );
        return Ok(report(&session, &differences, Outcome::Conflicts, summary));
    }
    let from_remote: Vec<&Difference<'_>> = differences
        .iter()
        .filter(|d| takes_remote(d, choices))
        .collect();
    let (setup_now, missing) = apply(paths, &session, &from_remote, prompts)?;
    set_base(paths, &session, &remote_commit)?;
    let kept_local = differences
        .iter()
        .filter(|d| {
            matches!(d.state, ItemState::LocalChange | ItemState::Conflict)
                && !takes_remote(d, choices)
        })
        .count();
    let summary = with_missing_secrets(
        format!(
            "applied {} remote changes; {kept_local} local changes remain for `harness-sync push`",
            from_remote.len()
        ),
        missing.len(),
    );
    let mut pulled = report(&session, &differences, Outcome::Ok, summary);
    pulled
        .items
        .extend(missing.iter().map(ItemReport::secret_missing));
    pulled.missing_executables = executables::missing(&setup_now, &paths.home);
    pulled.base_commit = Some(remote_commit);
    Ok(pulled)
}

fn set_base(paths: &Paths, session: &Session, remote_commit: &str) -> Result<()> {
    session.storage.set_base(remote_commit)?;
    if session.config.harnesses_without_base.is_empty() {
        return Ok(());
    }
    let config_file = paths.config_file();
    let mut config = Config::load_connected(&config_file)?;
    config.harnesses_without_base.clear();
    config.save(&config_file)
}

fn ask_sides(
    prompts: &dyn Prompts,
    differences: &[Difference<'_>],
    choices: &Choices<'_>,
) -> Result<Vec<Take>> {
    let mut takes = choices.takes.to_vec();
    let open = differences
        .iter()
        .filter(|d| d.state == ItemState::Conflict && choices.side_for(d.id).is_none());
    for difference in open {
        let Some(side) = prompts.ask_side(&ItemReport::from(difference))? else {
            break;
        };
        takes.push(Take {
            id: difference.id.clone(),
            side,
        });
    }
    Ok(takes)
}

fn apply(
    paths: &Paths,
    session: &Session,
    from_remote: &[&Difference<'_>],
    prompts: Option<&dyn Prompts>,
) -> Result<(Items, BTreeSet<KeyPath>)> {
    let mut missing = session.missing_secrets.clone();
    let mut ask = |key: &KeyPath| prompts.map_or(Ok(None), |prompts| prompts.ask_secret(key));
    let mut writes: Vec<(&ItemId, Option<Value>)> = Vec::new();
    for difference in from_remote {
        let Some(remote) = difference.remote else {
            writes.push((difference.id, None));
            continue;
        };
        let restored = secret::restore(
            difference.id,
            remote,
            session.local_setup.get(difference.id),
            &mut ask,
        )?;
        missing.extend(restored.missing);
        if let Some(mut value) = restored.value {
            home::expand(&mut value, &paths.home);
            writes.push((difference.id, Some(value)));
        }
    }
    let setup_now = if writes.is_empty() {
        session.local_setup.clone()
    } else {
        write(paths, session, &writes)?;
        setup::load_local(
            paths,
            session.storage.as_ref(),
            &session.declarations,
            &session.selection,
        )?
    };
    let missing = secret::still_missing(missing, &setup_now, &session.three_way.remote);
    if missing != session.missing_secrets {
        secret::save_missing(&paths.missing_secrets_file(), &missing)?;
    }
    Ok((setup_now, missing))
}

fn write(paths: &Paths, session: &Session, writes: &[(&ItemId, Option<Value>)]) -> Result<()> {
    let backup = Backup::new(paths);
    for keyed in keyed_files() {
        let edits: Vec<Edit<'_>> = writes
            .iter()
            .filter(|(id, _)| id.file == keyed.path && !id.is_whole_file())
            .map(|(id, value)| Edit {
                path: &id.path,
                value: value.as_ref(),
            })
            .collect();
        if !edits.is_empty() {
            setup::write_local(paths, &backup, keyed.path, &edits)?;
        }
    }
    let file_writes: Vec<(&ItemId, Option<&Value>)> = writes
        .iter()
        .filter(|(id, _)| id.is_whole_file())
        .map(|(id, value)| (*id, value.as_ref()))
        .collect();
    if !file_writes.is_empty() {
        files::write_local(paths, session.storage.as_ref(), &backup, &file_writes)?;
    }
    Ok(())
}

fn takes_remote(difference: &Difference<'_>, choices: &Choices<'_>) -> bool {
    match difference.state {
        ItemState::RemoteChange => true,
        ItemState::LocalChange | ItemState::Pinned | ItemState::SecretMissing => false,
        ItemState::Conflict => choices.side_for(difference.id) == Some(Side::Remote),
    }
}

fn report(
    session: &Session,
    differences: &[Difference<'_>],
    outcome: Outcome,
    summary: String,
) -> Report {
    Report {
        outcome,
        base_commit: session.base_commit.clone(),
        remote_commit: session.remote_commit.clone(),
        items: differences.iter().map(ItemReport::from).collect(),
        ..Report::new("pull", summary)
    }
}
