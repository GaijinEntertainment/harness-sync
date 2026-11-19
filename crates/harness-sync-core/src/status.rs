//! The `status` command.

use anyhow::Result;

use crate::compare::{Difference, ItemState};
use crate::paths::Paths;
use crate::report::{ItemReport, Report, counted, with_missing_secrets};
use crate::session::Session;
use crate::{retired, secret};

/// Lists the supported items that differ between the local setup and the remote snapshot of the primary target.
///
/// # Errors
///
/// Fails when this machine is not connected, the storage fails, or a harness file cannot be read.
pub fn run(paths: &Paths) -> Result<Report> {
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
        unmanaged_files: session.unmanaged_files()?,
        retired_items: retired::recorded(paths, &session)?,
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
        "{}, {}, {}, {} pinned",
        counted(count(ItemState::LocalChange), "local change"),
        counted(count(ItemState::RemoteChange), "remote change"),
        counted(count(ItemState::Conflict), "conflict"),
        count(ItemState::Pinned)
    )
}
