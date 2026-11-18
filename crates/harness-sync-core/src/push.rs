//! The `push` command, which sends the local setup to the primary target or to other targets.

use std::collections::BTreeSet;
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

use crate::compare::{Difference, ItemState};
use crate::config::{ALL_TARGETS, Config};
use crate::item::KeyPath;
use crate::paths::Paths;
use crate::report::{ItemReport, Outcome, Report, Review, TargetPush, short_commit};
use crate::session::Session;
use crate::storage::{FileChange, Publish};
use crate::{files, secret, setup};

/// Pushes to the primary target, or to the targets `to`, where `all` names every target. The push stops at the first
/// target that refuses it.
///
/// With `prune`, each push also removes the unmanaged files from the snapshot of its target.
///
/// # Errors
///
/// Fails when this machine is not connected, a target does not exist, a harness was added after the last pull, or
/// the storage fails.
pub fn run(paths: &Paths, to: &[String], prune: bool) -> Result<Report> {
    let config = Config::load_connected(&paths.config_file())?;
    let mut pushed = Vec::new();
    for target in chosen_targets(&config, to)? {
        let (session, report, review) =
            push_to(paths, &target, prune).with_context(|| failure(&target, &pushed))?;
        let stop = report.outcome != Outcome::Ok;
        pushed.push((session, report, review));
        if stop {
            break;
        }
    }
    Ok(combine(pushed))
}

fn push_to(paths: &Paths, target: &str, prune: bool) -> Result<(Session, Report, Option<Review>)> {
    let session = Session::open_target(paths, Some(target))?;
    let unmanaged = session.unmanaged_files()?;
    let pruned = if prune { unmanaged.as_slice() } else { &[] };
    let (mut report, review) = push(paths, &session, pruned)?;
    let snapshot_stayed = report.remote_commit == session.remote_commit;
    if pruned.is_empty() || snapshot_stayed {
        report.unmanaged_files = unmanaged;
    }
    Ok((session, report, review))
}

fn failure(target: &str, pushed: &[(Session, Report, Option<Review>)]) -> String {
    let earlier: Vec<String> = pushed
        .iter()
        .map(|(session, _, _)| format!("`{}`", session.target))
        .collect();
    if earlier.is_empty() {
        format!("cannot push to the target `{target}`")
    } else {
        format!(
            "cannot push to the target `{target}`; the push to {} ran before it",
            earlier.join(", ")
        )
    }
}

fn chosen_targets(config: &Config, to: &[String]) -> Result<Vec<String>> {
    if to.is_empty() {
        return Ok(vec![config.primary.clone()]);
    }
    let mut names: Vec<String> = if to.iter().any(|name| name == ALL_TARGETS) {
        config.targets.keys().cloned().collect()
    } else {
        for name in to {
            ensure!(
                config.targets.contains_key(name),
                "there is no target `{name}`; add it with `harness-sync target add`"
            );
        }
        to.to_vec()
    };
    let mut seen = BTreeSet::new();
    names.retain(|name| seen.insert(name.clone()));
    names.sort_by_key(|name| *name != config.primary);
    Ok(names)
}

fn push(paths: &Paths, session: &Session, pruned: &[String]) -> Result<(Report, Option<Review>)> {
    let differences = session.differences();
    if session.remote_commit != session.base_commit {
        return Ok((remote_ahead(session, &differences), None));
    }
    let added = &session.config.harnesses_without_base;
    ensure!(
        !session.is_primary() || added.is_empty(),
        "this machine started to sync {} after its last pull: run `harness-sync pull` first",
        added.join(", ")
    );
    let pushed_items = differences
        .iter()
        .filter(|d| d.state != ItemState::Pinned)
        .count();
    if pushed_items == 0 && pruned.is_empty() {
        let summary = if differences.is_empty() {
            "nothing to push: the local setup matches the remote snapshot"
        } else {
            "nothing to push: only pinned items differ from the remote snapshot"
        };
        return Ok((report(session, &differences, summary.to_owned()), None));
    }
    let outgoing = session.outgoing();
    let remote = &session.three_way.remote;
    let file_changes = files::changes(&outgoing, remote, paths)?;
    let mut found = secret::scan(&outgoing);
    found.extend(files::token_like(&file_changes));
    let mut changes = setup::json_changes(&outgoing, remote)?;
    changes.extend(file_changes);
    if !found.is_empty() {
        return Ok((secrets_found(session, &differences, &found), None));
    }
    changes.extend(
        pruned
            .iter()
            .map(|path| FileChange::Delete { path: path.clone() }),
    );
    let sent = sent(pushed_items, pruned.len());
    let commit = session
        .storage
        .commit(session.remote_commit.as_deref(), &changes, &subject())?;
    match session.storage.publish(&commit)? {
        Publish::Published => {}
        Publish::RemoteAhead if session.is_primary() => {
            return Ok((remote_ahead(session, &differences), None));
        }
        Publish::RemoteAhead => bail!(
            "the target got a commit during the push: run `harness-sync push --to {}` again",
            session.target
        ),
        Publish::SentForReview { change_url } => {
            let summary = format!(
                "sent {sent} for review as {}; the snapshot moves after the change merges",
                change_url
                    .as_deref()
                    .unwrap_or_else(|| short_commit(&commit))
            );
            let review = Review { commit, change_url };
            return Ok((report(session, &differences, summary), Some(review)));
        }
    }
    if session.is_primary() {
        session.storage.set_base(&commit)?;
    }
    let summary = format!("pushed {sent} as commit {}", short_commit(&commit));
    let mut pushed = report(session, &differences, summary);
    pushed.base_commit = Some(commit.clone());
    pushed.remote_commit = Some(commit);
    Ok((pushed, None))
}

fn sent(items: usize, pruned: usize) -> String {
    if pruned == 0 {
        format!("{items} items")
    } else {
        format!("{items} items and the removal of {pruned} unmanaged files")
    }
}

fn combine(pushed: Vec<(Session, Report, Option<Review>)>) -> Report {
    let summary = match pushed.as_slice() {
        [(_, report, _)] => report.summary.clone(),
        _ => pushed
            .iter()
            .map(|(session, report, _)| format!("{}: {}", session.target, report.summary))
            .collect::<Vec<_>>()
            .join("; "),
    };
    let mut combined = Report::new("push", summary);
    for (session, report, review) in pushed {
        combined.outcome = report.outcome;
        combined.secrets_found.extend(report.secrets_found);
        for marketplace in report.undeclared_marketplaces {
            if !combined.undeclared_marketplaces.contains(&marketplace) {
                combined.undeclared_marketplaces.push(marketplace);
            }
        }
        let mut target = TargetPush {
            target: session.target.clone(),
            outcome: report.outcome,
            commit: report.remote_commit.clone(),
            review,
            items: Vec::new(),
            unmanaged_files: Vec::new(),
        };
        if session.is_primary() {
            combined.base_commit = report.base_commit;
            combined.remote_commit = report.remote_commit;
            combined.items = report.items;
            combined.unmanaged_files = report.unmanaged_files;
        } else {
            target.items = report.items;
            target.unmanaged_files = report.unmanaged_files;
        }
        combined.pushes.push(target);
    }
    combined
}

fn report(session: &Session, differences: &[Difference<'_>], summary: String) -> Report {
    Report {
        base_commit: session.base_commit.clone(),
        remote_commit: session.remote_commit.clone(),
        items: differences.iter().map(ItemReport::from).collect(),
        undeclared_marketplaces: session.undeclared_marketplaces.clone(),
        ..Report::new("push", summary)
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

fn remote_ahead(session: &Session, differences: &[Difference<'_>]) -> Report {
    Report {
        outcome: Outcome::RemoteAhead,
        ..report(
            session,
            differences,
            "the remote snapshot has commits that this machine has not pulled: run `harness-sync pull` first"
                .to_owned(),
        )
    }
}

fn subject() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|host| !host.is_empty())
        .map_or_else(
            || "harness-sync push".to_owned(),
            |host| format!("harness-sync push from {host}"),
        )
}
