use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread;

use anyhow::{Context, Result, anyhow, bail};

pub(crate) const BASE_REF: &str = "refs/cc-sync/base";

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

    pub(crate) fn read_files(&self, specs: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        let mut input = specs.join("\n");
        input.push('\n');
        let output = self.run_with_input(&["cat-file", "--batch"], input.into_bytes())?;
        parse_batch(&output.stdout, specs.len())
    }

    fn run_with_input(&self, args: &[&str], input: Vec<u8>) -> Result<Output> {
        let subcommand = args.first().copied().unwrap_or_default();
        let mut child = self
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("cannot run git; is it installed and on PATH?")?;
        let mut stdin = child
            .stdin
            .take()
            .with_context(|| format!("cannot write to git {subcommand}"))?;
        let writer = thread::spawn(move || stdin.write_all(&input));
        let output = child
            .wait_with_output()
            .with_context(|| format!("cannot read from git {subcommand}"))?;
        writer
            .join()
            .map_err(|_| anyhow!("the writer to git {subcommand} panicked"))?
            .with_context(|| format!("cannot write to git {subcommand}"))?;
        if !output.status.success() {
            bail!(
                "git {subcommand} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(output)
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

fn parse_batch(mut stream: &[u8], count: usize) -> Result<Vec<Option<Vec<u8>>>> {
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        let end = stream
            .iter()
            .position(|&byte| byte == b'\n')
            .context("git cat-file output ended early")?;
        let header = String::from_utf8_lossy(&stream[..end]);
        stream = &stream[end + 1..];
        if header.ends_with(" missing") {
            files.push(None);
            continue;
        }
        let size: usize = header
            .rsplit(' ')
            .next()
            .and_then(|size| size.parse().ok())
            .with_context(|| format!("unexpected git cat-file header `{header}`"))?;
        let content = stream
            .get(..size)
            .context("git cat-file output ended inside a file")?;
        files.push(Some(content.to_vec()));
        stream = stream.get(size + 1..).unwrap_or_default();
    }
    Ok(files)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_output_yields_files_in_spec_order_and_none_for_missing_paths() {
        let stream = b"abc blob 3\none\nHEAD:gone missing\ndef blob 0\n\n";

        let files = parse_batch(stream, 3).unwrap();

        assert_eq!(files, [Some(b"one".to_vec()), None, Some(Vec::new())]);
    }

    #[test]
    fn batch_output_that_ends_early_is_an_error() {
        assert!(parse_batch(b"abc blob 10\nshort", 1).is_err());
    }
}
