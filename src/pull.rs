use std::collections::BTreeSet;

use anyhow::{Result, ensure};
use clap::ValueEnum;
use serde_json::Value;

use crate::compare::{Difference, ItemState};
use crate::item::{ItemId, KeyPath};
use crate::json_edit::Edit;
use crate::output;
use crate::paths::Paths;
use crate::report::{ItemReport, Outcome, Report, with_missing_secrets};
use crate::session::Session;
use crate::{secret, setup};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Side {
    Local,
    Remote,
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

pub(crate) fn parse_take(text: &str) -> Result<Take, String> {
    let (id, side) = text
        .rsplit_once('=')
        .ok_or_else(|| format!("`{text}` is not ITEM=local or ITEM=remote"))?;
    Ok(Take {
        id: id.parse().map_err(|err: anyhow::Error| err.to_string())?,
        side: Side::from_str(side, true)?,
    })
}

pub(crate) fn run(paths: &Paths, choices: &Choices<'_>, prompts: bool) -> Result<Report> {
    let session = Session::open(paths)?;
    let differences = session.three_way.differences();
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
    let taken: Vec<&Difference<'_>> = differences
        .iter()
        .filter(|d| takes_remote(d, choices))
        .collect();
    let missing = apply(paths, &session, &taken, prompts)?;
    session.git.set_base(&remote_commit)?;
    let summary = with_missing_secrets(
        format!(
            "applied {} remote changes; {} local changes remain for `cc-sync push`",
            taken.len(),
            differences.len() - taken.len()
        ),
        missing.len(),
    );
    let mut pulled = report(&session, &differences, Outcome::Ok, summary);
    pulled
        .items
        .extend(missing.iter().map(ItemReport::secret_missing));
    pulled.base_commit = Some(remote_commit);
    Ok(pulled)
}

fn apply(
    paths: &Paths,
    session: &Session,
    taken: &[&Difference<'_>],
    prompts: bool,
) -> Result<BTreeSet<KeyPath>> {
    let mut missing = session.missing_secrets.clone();
    let mut ask = |key: &KeyPath| {
        if prompts {
            output::ask_secret(key)
        } else {
            Ok(None)
        }
    };
    let mut writes: Vec<(&ItemId, Option<Value>)> = Vec::new();
    for difference in taken {
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
        if let Some(value) = restored.value {
            writes.push((difference.id, Some(value)));
        }
    }
    let setup_now = if writes.is_empty() {
        session.local_setup.clone()
    } else {
        let edits: Vec<Edit<'_>> = writes
            .iter()
            .map(|(id, value)| Edit {
                path: &id.path,
                value: value.as_ref(),
            })
            .collect();
        setup::write_local(paths, &edits)?;
        setup::load_local(paths)?
    };
    let missing = secret::still_missing(missing, &setup_now, &session.three_way.remote);
    if missing != session.missing_secrets {
        secret::save_missing(&paths.missing_secrets_file(), &missing)?;
    }
    Ok(missing)
}

fn takes_remote(difference: &Difference<'_>, choices: &Choices<'_>) -> bool {
    match difference.state {
        ItemState::RemoteChange => true,
        ItemState::LocalChange | ItemState::SecretMissing => false,
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
        command: "pull",
        outcome,
        base_commit: session.base_commit.clone(),
        remote_commit: session.remote_commit.clone(),
        items: differences.iter().map(ItemReport::from).collect(),
        secrets_found: Vec::new(),
        summary,
    }
}
