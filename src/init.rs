use std::collections::BTreeMap;
use std::fs;

use anyhow::{Context, Result, ensure};

use crate::config::{Config, Target};
use crate::git::{self, BASE_REF, Git};
use crate::harness::{self, HARNESSES};
use crate::paths::Paths;
use crate::report::{Report, short_commit};

pub(crate) fn run(
    paths: &Paths,
    name: &str,
    target: Target,
    harnesses: &[String],
) -> Result<Report> {
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
            pins: Vec::new(),
            targets: BTreeMap::from([(name.to_owned(), target)]),
        },
    };
    fs::create_dir_all(&paths.state_dir)
        .with_context(|| format!("cannot create {}", paths.state_dir.display()))?;
    let repo_dir = paths.repo_dir();
    let git = Git::open(repo_dir.clone());
    if !repo_dir.exists() {
        git.init_bare()?;
    }
    git.set_remote(&config.primary, &remote)?;
    git.fetch(&config.primary)?;
    config.save(&config_file)?;
    let remote_commit = git.resolve(&git::remote_ref(&config.primary, &branch))?;
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
        base_commit: git.resolve(BASE_REF)?,
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
    let present: Vec<String> = HARNESSES
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
