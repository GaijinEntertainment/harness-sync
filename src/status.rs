use anyhow::Result;

use crate::compare::{Difference, ItemState};
use crate::paths::Paths;
use crate::report::{ItemReport, Outcome, Report};
use crate::session::Session;

pub(crate) fn run(paths: &Paths) -> Result<Report> {
    let session = Session::open(paths)?;
    let differences = session.three_way.differences();
    Ok(Report {
        command: "status",
        outcome: Outcome::Ok,
        summary: summary(&differences),
        items: differences.iter().map(ItemReport::from).collect(),
        base_commit: session.base_commit,
        remote_commit: session.remote_commit,
    })
}

fn summary(differences: &[Difference<'_>]) -> String {
    if differences.is_empty() {
        return "the local setup matches the remote snapshot".to_owned();
    }
    let count = |state| differences.iter().filter(|d| d.state == state).count();
    format!(
        "{} local changes, {} remote changes, {} conflicts",
        count(ItemState::LocalChange),
        count(ItemState::RemoteChange),
        count(ItemState::Conflict)
    )
}
