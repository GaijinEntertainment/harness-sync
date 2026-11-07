use std::collections::BTreeSet;
use std::process::Command;

use anyhow::{Context, Result, ensure};

use crate::compare::{Difference, ItemState};
use crate::config::{ALL_TARGETS, Config};
use crate::git;
use crate::item::KeyPath;
use crate::paths::Paths;
use crate::report::{ItemReport, Outcome, Report, TargetPush, short_commit};
use crate::session::Session;
use crate::{files, secret, setup};

pub(crate) fn run(paths: &Paths, to: &[String]) -> Result<Report> {
    let config = Config::load_connected(&paths.config_file())?;
    let mut pushed = Vec::new();
    for target in chosen_targets(&config, to)? {
        let session = Session::open_target(paths, Some(&target))?;
        let report = push(paths, &session)
            .with_context(|| format!("cannot push to the target `{target}`"))?;
        let stop = report.outcome != Outcome::Ok;
        pushed.push((session, report));
        if stop {
            break;
        }
    }
    Ok(combine(pushed))
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
                "there is no target `{name}`; add it with `cc-sync target add`"
            );
        }
        to.to_vec()
    };
    let mut seen = BTreeSet::new();
    names.retain(|name| seen.insert(name.clone()));
    names.sort_by_key(|name| *name != config.primary);
    Ok(names)
}

fn push(paths: &Paths, session: &Session) -> Result<Report> {
    if session.remote_commit != session.base_commit {
        return Ok(remote_ahead(session));
    }
    let differences = session.differences();
    let pushed_items = differences
        .iter()
        .filter(|d| d.state != ItemState::Pinned)
        .count();
    if pushed_items == 0 {
        return Ok(report(
            session,
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
        return Ok(secrets_found(session, &differences, &found));
    }
    let commit = session
        .git
        .commit(session.remote_commit.as_deref(), &changes, &message())?;
    if let Err(err) = session.git.push(&session.target, &commit, &session.branch) {
        if session.is_primary() {
            session.git.fetch(&session.target)?;
            let remote_now = session
                .git
                .resolve(&git::remote_ref(&session.target, &session.branch))?;
            if remote_now != session.remote_commit {
                return Ok(remote_ahead(session));
            }
        }
        return Err(err);
    }
    if session.is_primary() {
        session.git.set_base(&commit)?;
    }
    let summary = format!(
        "pushed {pushed_items} items as commit {}",
        short_commit(&commit)
    );
    let mut pushed = report(session, &differences, summary);
    pushed.base_commit = Some(commit.clone());
    pushed.remote_commit = Some(commit);
    Ok(pushed)
}

fn combine(pushed: Vec<(Session, Report)>) -> Report {
    let summary = match pushed.as_slice() {
        [(_, report)] => report.summary.clone(),
        _ => pushed
            .iter()
            .map(|(session, report)| format!("{}: {}", session.target, report.summary))
            .collect::<Vec<_>>()
            .join("; "),
    };
    let mut combined = Report::new("push", summary);
    for (session, report) in pushed {
        combined.outcome = report.outcome;
        combined.secrets_found.extend(report.secrets_found);
        combined.undeclared_marketplaces = report.undeclared_marketplaces;
        let mut target = TargetPush {
            target: session.target.clone(),
            outcome: report.outcome,
            commit: report.remote_commit.clone(),
            items: Vec::new(),
        };
        if session.is_primary() {
            combined.base_commit = report.base_commit;
            combined.remote_commit = report.remote_commit;
            combined.items = report.items;
        } else {
            target.items = report.items;
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
