use std::fs;

use anyhow::{Context, Result, ensure};

use crate::config::Config;
use crate::git::{self, Git};
use crate::paths::Paths;
use crate::report::{Outcome, Report};

pub(crate) fn run(paths: &Paths, remote: &str, branch: &str) -> Result<Report> {
    let config = Config {
        remote: remote.to_owned(),
        branch: branch.to_owned(),
    };
    let config_file = paths.config_file();
    if let Some(existing) = Config::load(&config_file)? {
        ensure!(
            existing == config,
            "this machine is already connected to {} (branch {}); remove {} to connect it to another remote",
            existing.remote,
            existing.branch,
            paths.state_dir.display()
        );
    }
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
                short(commit)
            )
        },
    );
    Ok(Report {
        command: "init",
        outcome: Outcome::Ok,
        remote_commit,
        summary,
    })
}

fn short(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}
