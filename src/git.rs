use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, bail};

#[derive(Debug)]
pub(crate) struct Git {
    git_dir: PathBuf,
}

pub(crate) fn remote_ref(branch: &str) -> String {
    format!("refs/remotes/origin/{branch}")
}

impl Git {
    pub(crate) const fn open(git_dir: PathBuf) -> Self {
        Self { git_dir }
    }

    pub(crate) fn init_bare(&self) -> Result<()> {
        let mut command = Command::new("git");
        command
            .args(["init", "--bare", "--quiet"])
            .arg(&self.git_dir);
        run(command, "init").map(drop)
    }

    pub(crate) fn set_origin(&self, url: &str) -> Result<()> {
        self.run(&["config", "remote.origin.url", url])?;
        self.run(&[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ])
        .map(drop)
    }

    pub(crate) fn fetch(&self) -> Result<()> {
        self.run(&["fetch", "--quiet", "--prune", "origin"])
            .map(drop)
    }

    pub(crate) fn resolve(&self, refname: &str) -> Result<Option<String>> {
        let output = self
            .command(&[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{refname}^{{commit}}"),
            ])
            .output()
            .context("cannot run git")?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        ))
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command
            .arg("--git-dir")
            .arg(&self.git_dir)
            .args(["-c", "core.autocrlf=false"])
            .args(args)
            .stdin(Stdio::null());
        command
    }

    fn run(&self, args: &[&str]) -> Result<Output> {
        run(
            self.command(args),
            args.first().copied().unwrap_or_default(),
        )
    }
}

fn run(mut command: Command, subcommand: &str) -> Result<Output> {
    let output = command
        .output()
        .context("cannot run git; is it installed and on PATH?")?;
    if !output.status.success() {
        bail!(
            "git {subcommand} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output)
}
