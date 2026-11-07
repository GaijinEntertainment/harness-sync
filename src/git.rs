use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail, ensure};

pub(crate) const BASE_REF: &str = "refs/harness-sync/base";
const PENDING_REF: &str = "refs/harness-sync/pending";

#[derive(Debug)]
pub(crate) struct Git {
    git_dir: PathBuf,
}

#[derive(Debug)]
pub(crate) enum TreeChange {
    Write {
        path: String,
        content: Vec<u8>,
        executable: bool,
    },
    Delete {
        path: String,
    },
}

impl TreeChange {
    fn write_to(&self, stream: &mut Vec<u8>) -> Result<()> {
        let (Self::Write { path, .. } | Self::Delete { path }) = self;
        ensure!(
            !path.contains('\n') && !path.starts_with('"'),
            "cannot store the path {path:?} in the remote snapshot"
        );
        match self {
            Self::Write {
                content,
                executable,
                ..
            } => {
                let mode = if *executable { "100755" } else { "100644" };
                stream
                    .extend(format!("M {mode} inline {path}\ndata {}\n", content.len()).as_bytes());
                stream.extend(content);
                stream.push(b'\n');
            }
            Self::Delete { .. } => stream.extend(format!("D {path}\n").as_bytes()),
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TreeEntry {
    pub(crate) path: String,
    pub(crate) blob: String,
    pub(crate) executable: bool,
}

pub(crate) fn remote_ref(target: &str, branch: &str) -> String {
    format!("refs/remotes/{target}/{branch}")
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

    pub(crate) fn set_remote(&self, target: &str, url: &str) -> Result<()> {
        self.run(&["config", &format!("remote.{target}.url"), url])?;
        self.run(&[
            "config",
            &format!("remote.{target}.fetch"),
            &format!("+refs/heads/*:refs/remotes/{target}/*"),
        ])
        .map(drop)
    }

    pub(crate) fn remove_remote(&self, target: &str) -> Result<()> {
        self.run(&["remote", "remove", target]).map(drop)
    }

    pub(crate) fn fetch(&self, target: &str) -> Result<()> {
        self.run(&["fetch", "--quiet", "--prune", target]).map(drop)
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

    pub(crate) fn list_files(&self, commit: &str, dir: &str) -> Result<Vec<TreeEntry>> {
        let output = self.run(&["ls-tree", "-r", "-z", "--full-tree", commit, "--", dir])?;
        parse_tree(&output.stdout)
    }

    pub(crate) fn hash_files(&self, files: &[PathBuf]) -> Result<Vec<String>> {
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let mut input = Vec::new();
        for file in files {
            let path = file.to_str().with_context(|| {
                format!("cannot hash {}: the path is not UTF-8", file.display())
            })?;
            input.extend(path.as_bytes());
            input.push(b'\n');
        }
        let output =
            self.run_with_input(&["hash-object", "--no-filters", "--stdin-paths"], input)?;
        let blobs: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect();
        ensure!(
            blobs.len() == files.len(),
            "git hash-object returned {} ids for {} files",
            blobs.len(),
            files.len()
        );
        Ok(blobs)
    }

    pub(crate) fn commit(
        &self,
        parent: Option<&str>,
        changes: &[TreeChange],
        message: &str,
    ) -> Result<String> {
        let mut stream = format!(
            "commit {PENDING_REF}\ncommitter {}\ndata {}\n{message}\n",
            self.committer(),
            message.len()
        )
        .into_bytes();
        if let Some(parent) = parent {
            stream.extend(format!("from {parent}\n").as_bytes());
        }
        for change in changes {
            change.write_to(&mut stream)?;
        }
        self.run_with_input(&["fast-import", "--quiet", "--force"], stream)?;
        self.resolve(PENDING_REF)?
            .context("git fast-import did not create the snapshot commit")
    }

    pub(crate) fn push(&self, target: &str, commit: &str, destination: &str) -> Result<String> {
        let output = self.run(&[
            "push",
            "--quiet",
            target,
            &format!("{commit}:{destination}"),
        ])?;
        Ok(String::from_utf8_lossy(&output.stderr).into_owned())
    }

    pub(crate) fn hash_text(&self, text: &str) -> Result<String> {
        let output = self.run_with_input(&["hash-object", "--stdin"], text.as_bytes().to_vec())?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    pub(crate) fn set_base(&self, commit: &str) -> Result<()> {
        self.run(&["update-ref", BASE_REF, commit]).map(drop)
    }

    pub(crate) fn clear_base(&self) -> Result<()> {
        self.run(&["update-ref", "-d", BASE_REF]).map(drop)
    }

    fn committer(&self) -> String {
        self.run(&["var", "GIT_COMMITTER_IDENT"]).map_or_else(
            |_| {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |since| since.as_secs());
                format!("harness-sync <harness-sync@localhost> {now} +0000")
            },
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        )
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

fn parse_tree(stream: &[u8]) -> Result<Vec<TreeEntry>> {
    let mut entries = Vec::new();
    for record in stream
        .split(|&byte| byte == 0)
        .filter(|record| !record.is_empty())
    {
        let record =
            std::str::from_utf8(record).context("git ls-tree printed a path that is not UTF-8")?;
        let (meta, path) = record
            .split_once('\t')
            .with_context(|| format!("unexpected git ls-tree record `{record}`"))?;
        let fields: Vec<&str> = meta.split(' ').collect();
        let [mode, kind, blob] = fields[..] else {
            bail!("unexpected git ls-tree record `{record}`");
        };
        if kind == "blob" {
            entries.push(TreeEntry {
                path: path.to_owned(),
                blob: blob.to_owned(),
                executable: mode == "100755",
            });
        }
    }
    Ok(entries)
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
    fn tree_records_yield_blobs_with_their_mode() {
        let stream = b"100644 blob aaa\tfiles/CLAUDE.md\x00100755 blob bbb\tfiles/hooks/x.sh\x00160000 commit ccc\tfiles/sub\x00";

        let entries = parse_tree(stream).unwrap();

        assert_eq!(
            entries,
            [
                TreeEntry {
                    path: "files/CLAUDE.md".to_owned(),
                    blob: "aaa".to_owned(),
                    executable: false
                },
                TreeEntry {
                    path: "files/hooks/x.sh".to_owned(),
                    blob: "bbb".to_owned(),
                    executable: true
                },
            ]
        );
    }

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
