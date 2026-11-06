use std::process::Command;

use anyhow::Result;

use crate::compare::Difference;
use crate::git;
use crate::paths::Paths;
use crate::report::{ItemReport, Outcome, Report, short_commit};
use crate::session::Session;
use crate::setup;

pub(crate) fn run(paths: &Paths) -> Result<Report> {
    let session = Session::open(paths)?;
    if session.remote_commit != session.base_commit {
        return Ok(remote_ahead(&session));
    }
    let differences = session.three_way.differences();
    if differences.is_empty() {
        return Ok(report(
            &session,
            &differences,
            "nothing to push: the local setup matches the remote snapshot".to_owned(),
        ));
    }
    let files = setup::snapshot_files(&session.three_way.local)?;
    let commit = session
        .git
        .commit(session.remote_commit.as_deref(), &files, &message())?;
    if let Err(err) = session.git.push(&commit, &session.config.branch) {
        session.git.fetch()?;
        let remote_now = session
            .git
            .resolve(&git::remote_ref(&session.config.branch))?;
        if remote_now != session.remote_commit {
            return Ok(remote_ahead(&session));
        }
        return Err(err);
    }
    session.git.set_base(&commit)?;
    let summary = format!(
        "pushed {} items as commit {}",
        differences.len(),
        short_commit(&commit)
    );
    let mut pushed = report(&session, &differences, summary);
    pushed.base_commit = Some(commit.clone());
    pushed.remote_commit = Some(commit);
    Ok(pushed)
}

fn report(session: &Session, differences: &[Difference<'_>], summary: String) -> Report {
    Report {
        command: "push",
        outcome: Outcome::Ok,
        base_commit: session.base_commit.clone(),
        remote_commit: session.remote_commit.clone(),
        items: differences.iter().map(ItemReport::from).collect(),
        summary,
    }
}

fn remote_ahead(session: &Session) -> Report {
    Report {
        outcome: Outcome::RemoteAhead,
        ..report(
            session,
            &[],
            "the remote snapshot has commits that this machine has not pulled: run `cc-sync pull` first"
                .to_owned(),
        )
    }
}

fn message() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|host| !host.is_empty())
        .map_or_else(
            || "cc-sync push".to_owned(),
            |host| format!("cc-sync push from {host}"),
        )
}
