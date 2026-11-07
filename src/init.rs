use std::fs;

use anyhow::{Context, Result, ensure};

use crate::config::Config;
use crate::git::{self, BASE_REF, Git};
use crate::paths::Paths;
use crate::report::{Outcome, Report, short_commit};

pub(crate) fn run(paths: &Paths, remote: &str, branch: &str) -> Result<Report> {
    let config_file = paths.config_file();
    let existing = Config::load(&config_file)?;
    if let Some(existing) = &existing {
        ensure!(
            existing.remote == remote && existing.branch == branch,
            "this machine is already connected to {} (branch {}); remove {} to connect it to another remote",
            existing.remote,
            existing.branch,
            paths.state_dir.display()
        );
    }
    let config = existing.unwrap_or_else(|| Config {
        remote: remote.to_owned(),
        branch: branch.to_owned(),
        pins: Vec::new(),
    });
    fs::create_dir_all(&paths.state_dir)
        .with_context(|| format!("cannot create {}", paths.state_dir.display()))?;
    let repo_dir = paths.repo_dir();
    let git = Git::open(repo_dir.clone());
    if !repo_dir.exists() {
        git.init_bare()?;
    }
    git.set_origin(remote)?;
    git.fetch()?;
    config.save(&config_file)?;
    let remote_commit = git.resolve(&git::remote_ref(branch))?;
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
        command: "init",
        outcome: Outcome::Ok,
        base_commit: git.resolve(BASE_REF)?,
        remote_commit,
        items: Vec::new(),
        secrets_found: Vec::new(),
        missing_executables: Vec::new(),
        undeclared_marketplaces: Vec::new(),
        summary,
    })
}
