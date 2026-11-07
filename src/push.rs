use std::process::Command;

use anyhow::Result;

use crate::compare::{Difference, ItemState};
use crate::git;
use crate::item::KeyPath;
use crate::paths::Paths;
use crate::report::{ItemReport, Outcome, Report, short_commit};
use crate::session::Session;
use crate::{files, secret, setup};

pub(crate) fn run(paths: &Paths) -> Result<Report> {
    let session = Session::open(paths)?;
    if session.remote_commit != session.base_commit {
        return Ok(remote_ahead(&session));
    }
    let differences = session.differences();
    let pushed_items = differences
        .iter()
        .filter(|d| d.state != ItemState::Pinned)
        .count();
    if pushed_items == 0 {
        return Ok(report(
            &session,
            &differences,
            "nothing to push: the local setup matches the remote snapshot".to_owned(),
        ));
    }
    let outgoing = session.outgoing();
    let mut changes = setup::json_changes(&outgoing)?;
    changes.extend(files::changes(
        &outgoing,
        &session.three_way.remote,
        &paths.claude_dir,
    )?);
    let mut found = secret::scan(&outgoing);
    found.extend(files::token_like(&changes));
    if !found.is_empty() {
        return Ok(secrets_found(&session, &differences, &found));
    }
    let commit = session
        .git
        .commit(session.remote_commit.as_deref(), &changes, &message())?;
    let target = &session.config.primary;
    let branch = &session.config.primary()?.branch;
    if let Err(err) = session.git.push(target, &commit, branch) {
        session.git.fetch(target)?;
        let remote_now = session.git.resolve(&git::remote_ref(target, branch))?;
        if remote_now != session.remote_commit {
            return Ok(remote_ahead(&session));
        }
        return Err(err);
    }
    session.git.set_base(&commit)?;
    let summary = format!(
        "pushed {pushed_items} items as commit {}",
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
        secrets_found: Vec::new(),
        missing_executables: Vec::new(),
        undeclared_marketplaces: session.undeclared_marketplaces.clone(),
        summary,
    }
}

fn secrets_found(session: &Session, differences: &[Difference<'_>], found: &[KeyPath]) -> Report {
    let paths: Vec<String> = found.iter().map(ToString::to_string).collect();
    let summary = format!(
        "push blocked: token-like values outside secret placeholders at {}; remove each value or keep it \
         under a key that a secret name rule matches",
        paths.join(", ")
    );
    Report {
        outcome: Outcome::SecretsFound,
        secrets_found: paths,
        ..report(session, differences, summary)
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
