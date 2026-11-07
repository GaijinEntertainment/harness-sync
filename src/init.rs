use std::collections::BTreeMap;
use std::fs;

use anyhow::{Context, Result, ensure};

use crate::config::{Config, Target};
use crate::git::{self, BASE_REF, Git};
use crate::paths::Paths;
use crate::report::{Report, short_commit};

pub(crate) fn run(paths: &Paths, name: &str, target: Target) -> Result<Report> {
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
    let config = existing.unwrap_or_else(|| Config {
        primary: name.to_owned(),
        pins: Vec::new(),
        targets: BTreeMap::from([(name.to_owned(), target)]),
    });
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
        || format!("connected to {remote}; branch {branch} is empty: run `cc-sync push` to create it"),
        |commit| {
            format!(
                "connected to {remote}; branch {branch} is at {}: run `cc-sync pull --prefer remote` to take it",
                short_commit(commit)
            )
        },
    );
    Ok(Report {
        base_commit: git.resolve(BASE_REF)?,
        remote_commit,
        ..Report::new("init", summary)
    })
}
