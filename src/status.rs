use anyhow::Result;

use crate::compare::{Difference, ItemState};
use crate::paths::Paths;
use crate::report::{ItemReport, Report, with_missing_secrets};
use crate::secret;
use crate::session::Session;

pub(crate) fn run(paths: &Paths) -> Result<Report> {
    let session = Session::open(paths)?;
    let differences = session.differences();
    let missing = secret::still_missing(
        session.missing_secrets.iter().cloned(),
        &session.local_setup,
        &session.three_way.remote,
    );
    let summary = with_missing_secrets(summary(&differences), missing.len());
    Ok(Report {
        items: differences
            .iter()
            .map(ItemReport::from)
            .chain(missing.iter().map(ItemReport::secret_missing))
            .collect(),
        undeclared_marketplaces: session.undeclared_marketplaces,
        base_commit: session.base_commit,
        remote_commit: session.remote_commit,
        ..Report::new("status", summary)
    })
}

fn summary(differences: &[Difference<'_>]) -> String {
    if differences.is_empty() {
        return "the local setup matches the remote snapshot".to_owned();
    }
    let count = |state| differences.iter().filter(|d| d.state == state).count();
    format!(
        "{} local changes, {} remote changes, {} conflicts, {} pinned",
        count(ItemState::LocalChange),
        count(ItemState::RemoteChange),
        count(ItemState::Conflict),
        count(ItemState::Pinned)
    )
}
