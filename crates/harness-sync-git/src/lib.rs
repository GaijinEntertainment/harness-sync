//! The git and Gerrit storage kinds of harness-sync, run through the git executable (ADR 1.7, ADR 1.11).

mod gerrit;

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail, ensure};
use harness_sync_core::config::Target;
use harness_sync_core::paths::Paths;
use harness_sync_core::storage::{self, FileChange, Publish, SnapshotFile, Storage, StorageKind};

const BASE_REF: &str = "refs/harness-sync/base";
const PENDING_REF: &str = "refs/harness-sync/pending";

/// A branch of a git repo: a publish pushes the commit to the branch.
pub static GIT: StorageKind = StorageKind {
    name: storage::DEFAULT_KIND,
    open: |paths, name, target| GitTarget::open(paths, name, target, false),
};

/// A branch of a Gerrit project: a publish sends the commit for review, and the branch moves when the change merges.
pub static GERRIT: StorageKind = StorageKind {
    name: "gerrit",
    open: |paths, name, target| GitTarget::open(paths, name, target, true),
};

#[derive(Debug)]
struct GitTarget {
    git: Git,
    name: String,
    url: String,
    branch: String,
    gerrit: bool,
}

impl GitTarget {
    fn open(paths: &Paths, name: &str, target: &Target, gerrit: bool) -> Box<dyn Storage> {
        Box::new(Self {
            git: Git {
                git_dir: paths.repo_dir(),
            },
            name: name.to_owned(),
            url: target.url.clone(),
            branch: target.branch.clone(),
            gerrit,
        })
    }
}

impl Storage for GitTarget {
    fn connect(&self) -> Result<()> {
        if !self.git.git_dir.exists() {
            self.git.init_bare()?;
        }
        self.git.set_remote(&self.name, &self.url)
    }

    fn disconnect(&self) -> Result<()> {
        self.git.run(&["remote", "remove", &self.name]).map(drop)
    }

    fn fetch(&self) -> Result<()> {
        self.git
            .run(&["fetch", "--quiet", "--prune", &self.name])
            .map(drop)
    }

    fn head(&self) -> Result<Option<String>> {
        self.git
            .resolve(&format!("refs/remotes/{}/{}", self.name, self.branch))
    }

    fn read_files(&self, commit: &str, paths: &[&str]) -> Result<Vec<Option<Vec<u8>>>> {
        let specs: Vec<String> = paths
            .iter()
            .map(|path| format!("{commit}:{path}"))
            .collect();
        self.git.read_objects(&specs)
    }

    fn list_files(&self, commit: &str) -> Result<Vec<SnapshotFile>> {
        let output = self
            .git
            .run(&["ls-tree", "-r", "-z", "--full-tree", commit])?;
        parse_tree(&output.stdout)
    }

    fn read_contents(&self, ids: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        self.git.read_objects(ids)
    }

    fn content_ids(&self, files: &[PathBuf]) -> Result<Vec<String>> {
        self.git.hash_files(files)
    }

    fn commit(
        &self,
        parent: Option<&str>,
        changes: &[FileChange],
        subject: &str,
    ) -> Result<String> {
        let message = if self.gerrit {
            gerrit::message(&self.git, subject, parent)?
        } else {
            subject.to_owned()
        };
        self.git.commit(parent, changes, &message)
    }

    fn publish(&self, commit: &str) -> Result<Publish> {
        let destination = if self.gerrit {
            format!("refs/for/{}", self.branch)
        } else {
            format!("refs/heads/{}", self.branch)
        };
        let pushed = self.git.run(&[
            "push",
            "--quiet",
            &self.name,
            &format!("{commit}:{destination}"),
        ]);
        match pushed {
            Ok(output) if self.gerrit => Ok(Publish::SentForReview {
                change_url: gerrit::change_url(&String::from_utf8_lossy(&output.stderr)),
            }),
            Ok(_) => Ok(Publish::Published),
            Err(err) if self.gerrit => Err(err),
            Err(err) => {
                let parent = self.git.resolve(&format!("{commit}^"))?;
                self.fetch()?;
                if self.head()? == parent {
                    return Err(err);
                }
                Ok(Publish::RemoteAhead)
            }
        }
    }

    fn base(&self) -> Result<Option<String>> {
        self.git.resolve(BASE_REF)
    }

    fn set_base(&self, commit: &str) -> Result<()> {
        self.git.run(&["update-ref", BASE_REF, commit]).map(drop)
    }

    fn clear_base(&self) -> Result<()> {
        self.git.run(&["update-ref", "-d", BASE_REF]).map(drop)
    }
}

#[derive(Debug)]
struct Git {
    git_dir: PathBuf,
}

impl Git {
    fn init_bare(&self) -> Result<()> {
        let mut command = Command::new("git");
        command
            .args(["init", "--bare", "--quiet"])
            .arg(&self.git_dir);
        run(command, "init").map(drop)
    }

    fn set_remote(&self, target: &str, url: &str) -> Result<()> {
        self.run(&["config", &format!("remote.{target}.url"), url])?;
        self.run(&[
            "config",
            &format!("remote.{target}.fetch"),
            &format!("+refs/heads/*:refs/remotes/{target}/*"),
        ])
        .map(drop)
    }

    fn resolve(&self, refname: &str) -> Result<Option<String>> {
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

    fn read_objects(&self, specs: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        let mut input = specs.join("\n");
        input.push('\n');
        let output = self.run_with_input(&["cat-file", "--batch"], input.into_bytes())?;
        parse_batch(&output.stdout, specs.len())
    }

    fn hash_files(&self, files: &[PathBuf]) -> Result<Vec<String>> {
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

    fn commit(
        &self,
        parent: Option<&str>,
        changes: &[FileChange],
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
            write_change(change, &mut stream)?;
        }
        self.run_with_input(&["fast-import", "--quiet", "--force"], stream)?;
        self.resolve(PENDING_REF)?
            .context("git fast-import did not create the snapshot commit")
    }

    fn hash_text(&self, text: &str) -> Result<String> {
        let output = self.run_with_input(&["hash-object", "--stdin"], text.as_bytes().to_vec())?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
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

fn write_change(change: &FileChange, stream: &mut Vec<u8>) -> Result<()> {
    match change {
        FileChange::Write {
            path,
            content,
            executable,
        } => {
            ensure!(
                !path.contains('\n') && !path.starts_with('"'),
                "cannot store the path {path:?} in the remote snapshot"
            );
            let mode = if *executable { "100755" } else { "100644" };
            stream.extend(format!("M {mode} inline {path}\ndata {}\n", content.len()).as_bytes());
            stream.extend(content);
            stream.push(b'\n');
        }
        FileChange::Delete { path } => {
            stream.extend(format!("D {}\n", quoted(path)).as_bytes());
        }
    }
    Ok(())
}

// constraint: git fast-import reads a path in double quotes as a C-style string, which can hold a line break
fn quoted(path: &str) -> String {
    let mut quoted = String::from('"');
    for character in path.chars() {
        match character {
            '\n' => quoted.push_str("\\n"),
            '"' | '\\' => quoted.extend(['\\', character]),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
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

fn parse_tree(stream: &[u8]) -> Result<Vec<SnapshotFile>> {
    let mut files = Vec::new();
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
            files.push(SnapshotFile {
                path: path.to_owned(),
                content_id: blob.to_owned(),
                executable: mode == "100755",
            });
        }
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
    fn a_push_rejected_after_another_fetch_moved_the_tracking_ref_is_remote_ahead() {
        let dir = tempfile::tempdir().unwrap();
        let remote = Git {
            git_dir: dir.path().join("remote.git"),
        };
        remote.init_bare().unwrap();
        let machine = |repo: &str| GitTarget {
            git: Git {
                git_dir: dir.path().join(repo),
            },
            name: "origin".to_owned(),
            url: remote.git_dir.to_string_lossy().into_owned(),
            branch: "main".to_owned(),
            gerrit: false,
        };
        let write = |text: &str| {
            [FileChange::Write {
                path: "a.md".to_owned(),
                content: text.as_bytes().to_vec(),
                executable: false,
            }]
        };
        let (first, second) = (machine("first.git"), machine("second.git"));
        first.connect().unwrap();
        let base = first.commit(None, &write("one"), "one").unwrap();
        assert_eq!(first.publish(&base).unwrap(), Publish::Published);
        second.connect().unwrap();
        second.fetch().unwrap();
        let theirs = second.commit(Some(&base), &write("two"), "two").unwrap();
        assert_eq!(second.publish(&theirs).unwrap(), Publish::Published);
        let ours = first.commit(Some(&base), &write("three"), "three").unwrap();
        first.fetch().unwrap();

        let published = first.publish(&ours).unwrap();

        assert_eq!(published, Publish::RemoteAhead);
    }

    #[test]
    fn tree_records_yield_blobs_with_their_mode() {
        let stream = b"100644 blob aaa\t.claude/CLAUDE.md\x00100755 blob bbb\t.claude/hooks/x.sh\x00160000 commit ccc\t.claude/sub\x00";

        let files = parse_tree(stream).unwrap();

        assert_eq!(
            files,
            [
                SnapshotFile {
                    path: ".claude/CLAUDE.md".to_owned(),
                    content_id: "aaa".to_owned(),
                    executable: false
                },
                SnapshotFile {
                    path: ".claude/hooks/x.sh".to_owned(),
                    content_id: "bbb".to_owned(),
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
