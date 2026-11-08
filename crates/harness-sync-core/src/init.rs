//! The `init` command, which connects this machine to its primary target.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{Context, Result, ensure};

use crate::config::{Config, Target};
use crate::harness;
use crate::paths::Paths;
use crate::report::{Report, short_commit};
use crate::storage;

/// Connects this machine to `target` as its primary target `name`, and makes it sync `harnesses`, or each harness
/// whose directory exists when `harnesses` is empty.
///
/// # Errors
///
/// Fails when the URL of `target` holds a password, this machine is connected to another primary target, a harness
/// is unknown, no harness directory exists, or the storage cannot be set up or fetched.
pub fn run(paths: &Paths, name: &str, target: Target, harnesses: &[String]) -> Result<Report> {
    target.ensure_no_password()?;
    let config_file = paths.config_file();
    let remote = target.url.clone();
    let branch = target.branch.clone();
    let existing = Config::load(&config_file)?;
    if let Some(existing) = &existing {
        let primary = existing.primary()?;
        ensure!(
            existing.primary == name && *primary == target,
            "this machine is already connected to {} (branch {}); remove {} to connect it to another remote",
            primary.url,
            primary.branch,
            paths.state_dir.display()
        );
    }
    let config = match existing {
        Some(existing) => existing,
        None => Config {
            primary: name.to_owned(),
            harnesses: chosen_harnesses(paths, harnesses)?,
            harnesses_without_base: Vec::new(),
            pins: BTreeSet::new(),
            targets: BTreeMap::from([(name.to_owned(), target)]),
        },
    };
    fs::create_dir_all(&paths.state_dir)
        .with_context(|| format!("cannot create {}", paths.state_dir.display()))?;
    let storage = storage::open(paths, &config, &config.primary)?;
    storage.connect()?;
    storage.fetch()?;
    config.save(&config_file)?;
    let remote_commit = storage.head()?;
    let summary = remote_commit.as_deref().map_or_else(
        || format!("connected to {remote}; branch {branch} is empty: run `harness-sync push` to create it"),
        |commit| {
            format!(
                "connected to {remote}; branch {branch} is at {}: run `harness-sync pull --prefer remote` to take it",
                short_commit(commit)
            )
        },
    );
    Ok(Report {
        base_commit: storage.base()?,
        remote_commit,
        ..Report::new(
            "init",
            format!("{summary}; syncs {}", config.harnesses.join(", ")),
        )
    })
}

fn chosen_harnesses(paths: &Paths, named: &[String]) -> Result<Vec<String>> {
    if !named.is_empty() {
        for name in named {
            harness::by_name(name)?;
        }
        let mut chosen = named.to_vec();
        chosen.sort();
        chosen.dedup();
        return Ok(chosen);
    }
    let present: Vec<String> = harness::all()
        .iter()
        .filter(|harness| harness.is_present(paths))
        .map(|harness| harness.name.to_owned())
        .collect();
    ensure!(
        !present.is_empty(),
        "no harness directory exists on this machine; name the harnesses to sync with --harness"
    );
    Ok(present)
}
