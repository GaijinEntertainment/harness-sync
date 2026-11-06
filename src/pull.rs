use anyhow::{Result, ensure};
use clap::ValueEnum;
use serde_json::Value;

use crate::compare::{Difference, ItemState};
use crate::item::ItemId;
use crate::paths::Paths;
use crate::report::{ItemReport, Outcome, Report};
use crate::session::Session;
use crate::setup;

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

pub(crate) fn run(paths: &Paths, choices: &Choices<'_>) -> Result<Report> {
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
    let writes: Vec<(&ItemId, Option<&Value>)> = differences
        .iter()
        .filter(|d| takes_remote(d, choices))
        .map(|d| (d.id, d.remote))
        .collect();
    if !writes.is_empty() {
        setup::write_local(paths, &writes)?;
    }
    session.git.set_base(&remote_commit)?;
    let summary = format!(
        "applied {} remote changes; {} local changes remain for `cc-sync push`",
        writes.len(),
        differences.len() - writes.len()
    );
    let mut pulled = report(&session, &differences, Outcome::Ok, summary);
    pulled.base_commit = Some(remote_commit);
    Ok(pulled)
}

fn takes_remote(difference: &Difference<'_>, choices: &Choices<'_>) -> bool {
    match difference.state {
        ItemState::RemoteChange => true,
        ItemState::LocalChange => false,
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
        summary,
    }
}
