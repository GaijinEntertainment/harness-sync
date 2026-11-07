use std::collections::BTreeSet;
use std::process::{self, Command};
use std::time::SystemTime;

use anyhow::{Context, Result, ensure};

use crate::compare::{Difference, ItemState};
use crate::config::{ALL_TARGETS, Config, Kind};
use crate::git;
use crate::item::KeyPath;
use crate::paths::Paths;
use crate::report::{ItemReport, Outcome, Report, Review, TargetPush, short_commit};
use crate::session::Session;
use crate::{files, secret, setup};

pub(crate) fn run(paths: &Paths, to: &[String]) -> Result<Report> {
    let config = Config::load_connected(&paths.config_file())?;
    let mut pushed = Vec::new();
    for target in chosen_targets(&config, to)? {
        let session = Session::open_target(paths, Some(&target))?;
        let (report, review) = push(paths, &session)
            .with_context(|| format!("cannot push to the target `{target}`"))?;
        let stop = report.outcome != Outcome::Ok;
        pushed.push((session, report, review));
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

fn push(paths: &Paths, session: &Session) -> Result<(Report, Option<Review>)> {
    if session.remote_commit != session.base_commit {
        return Ok((remote_ahead(session), None));
    }
    let differences = session.differences();
    let pushed_items = differences
        .iter()
        .filter(|d| d.state != ItemState::Pinned)
        .count();
    if pushed_items == 0 {
        let summary = "nothing to push: the local setup matches the remote snapshot".to_owned();
        return Ok((report(session, &differences, summary), None));
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
        return Ok((secrets_found(session, &differences, &found), None));
    }
    let parent = session.remote_commit.as_deref();
    let commit = session.git.commit(parent, &changes, &message(session)?)?;
    let gerrit = session.kind == Kind::Gerrit;
    let destination = if gerrit {
        format!("refs/for/{}", session.branch)
    } else {
        format!("refs/heads/{}", session.branch)
    };
    let remote_messages = match session.git.push(&session.target, &commit, &destination) {
        Ok(messages) => messages,
        Err(err) => {
            if session.is_primary() && !gerrit {
                session.git.fetch(&session.target)?;
                let remote_now = session
                    .git
                    .resolve(&git::remote_ref(&session.target, &session.branch))?;
                if remote_now != session.remote_commit {
                    return Ok((remote_ahead(session), None));
                }
            }
            return Err(err);
        }
    };
    if gerrit {
        let review = Review {
            change_url: change_url(&remote_messages),
            commit: commit.clone(),
        };
        let summary = format!(
            "sent {pushed_items} items for review as {}; the snapshot moves after the change merges",
            review
                .change_url
                .as_deref()
                .unwrap_or_else(|| short_commit(&commit))
        );
        return Ok((report(session, &differences, summary), Some(review)));
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
    Ok((pushed, None))
}

fn change_url(remote_messages: &str) -> Option<String> {
    remote_messages
        .lines()
        .filter_map(|line| line.strip_prefix("remote:"))
        .flat_map(str::split_whitespace)
        .find(|word| word.starts_with("https://") || word.starts_with("http://"))
        .map(str::to_owned)
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
        combined.undeclared_marketplaces = report.undeclared_marketplaces;
        let mut target = TargetPush {
            target: session.target.clone(),
            outcome: report.outcome,
            commit: report.remote_commit.clone(),
            review,
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

fn message(session: &Session) -> Result<String> {
    let subject = Command::new("hostname")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|host| !host.is_empty())
        .map_or_else(
            || "cc-sync push".to_owned(),
            |host| format!("cc-sync push from {host}"),
        );
    if session.kind != Kind::Gerrit {
        return Ok(subject);
    }
    let seed = format!(
        "{subject}\n{}\n{:?}\n{}",
        session.remote_commit.as_deref().unwrap_or_default(),
        SystemTime::now(),
        process::id()
    );
    Ok(format!(
        "{subject}\n\nChange-Id: I{}",
        session.git.hash_text(&seed)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_change_url_comes_from_the_remote_messages_of_gerrit() {
        let messages = "remote: \nremote: Processing changes: refs: 1, new: 1, done\nremote: New Changes:        \n\
                        remote:   https://gerrit.example/c/setup/+/42 cc-sync push from host [NEW]        \n";

        assert_eq!(
            change_url(messages).as_deref(),
            Some("https://gerrit.example/c/setup/+/42")
        );
        assert_eq!(change_url("remote: Resolving deltas\n"), None);
    }
}
