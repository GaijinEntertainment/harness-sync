use std::fmt::Debug;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::{Config, Target};
use crate::paths::Paths;
use crate::registry;

pub(crate) const DEFAULT_KIND: &str = "git";

/// A storage kind that a target can name in its `kind`.
#[derive(Debug)]
pub(crate) struct StorageKind {
    pub(crate) name: &'static str,
    pub(crate) open: fn(&Paths, &str, &Target) -> Box<dyn Storage>,
}

/// The remote snapshot of one target. Commit ids and content ids are strings that the storage kind defines; the
/// core compares them only for equality.
pub(crate) trait Storage: Debug {
    /// Sets up this machine's access to the target. Setting it up again changes nothing.
    fn connect(&self) -> Result<()>;

    /// Removes this machine's access to the target.
    fn disconnect(&self) -> Result<()>;

    /// Updates this machine's view of the target.
    fn fetch(&self) -> Result<()>;

    /// The latest commit of the target as the last fetch saw it, or `None` when the target holds no snapshot.
    fn head(&self) -> Result<Option<String>>;

    /// The content of each file at `paths` in the snapshot of `commit`, or `None` for a path that it does not hold.
    fn read_files(&self, commit: &str, paths: &[&str]) -> Result<Vec<Option<Vec<u8>>>>;

    /// Every file in the snapshot of `commit`.
    fn list_files(&self, commit: &str) -> Result<Vec<SnapshotFile>>;

    /// The content of each content id that a snapshot file carries, or `None` for an id that this machine lacks.
    fn read_contents(&self, ids: &[String]) -> Result<Vec<Option<Vec<u8>>>>;

    /// The content id that each local file would have in a snapshot.
    fn content_ids(&self, files: &[PathBuf]) -> Result<Vec<String>>;

    /// Writes a commit of `changes` on `parent`, with `subject` as the first line of its message, and returns its id.
    /// The commit is not published.
    fn commit(&self, parent: Option<&str>, changes: &[FileChange], subject: &str)
    -> Result<String>;

    /// Publishes `commit`, whose parent is the head of the last fetch.
    fn publish(&self, commit: &str) -> Result<Publish>;

    /// The base commit, which this storage records for the primary target.
    fn base(&self) -> Result<Option<String>>;

    /// Records `commit` as the base commit and keeps its snapshot readable.
    fn set_base(&self, commit: &str) -> Result<()>;

    /// Drops the base commit.
    fn clear_base(&self) -> Result<()>;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Publish {
    /// The commit is the latest commit of the target.
    Published,
    /// The commit waits for review, and the target stays as it was.
    SentForReview { change_url: Option<String> },
    /// The target has a commit that the last fetch did not see; nothing was published.
    RemoteAhead,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SnapshotFile {
    pub(crate) path: String,
    pub(crate) content_id: String,
    pub(crate) executable: bool,
}

#[derive(Debug)]
pub(crate) enum FileChange {
    Write {
        path: String,
        content: Vec<u8>,
        executable: bool,
    },
    Delete {
        path: String,
    },
}

/// Opens the storage of the target `name` in the sync configuration.
pub(crate) fn open(paths: &Paths, config: &Config, name: &str) -> Result<Box<dyn Storage>> {
    let target = config.targets.get(name).with_context(|| {
        format!("there is no target `{name}`; add it with `harness-sync target add`")
    })?;
    open_target(paths, name, target)
}

/// Opens the storage of `target` under `name`, which the sync configuration does not need to hold yet.
pub(crate) fn open_target(paths: &Paths, name: &str, target: &Target) -> Result<Box<dyn Storage>> {
    let kinds = registry::installed().storage_kinds;
    let kind = kinds
        .iter()
        .find(|kind| kind.name == target.kind)
        .with_context(|| {
            let known: Vec<&str> = kinds.iter().map(|kind| kind.name).collect();
            format!(
                "the target `{name}` has the storage kind `{}`; this harness-sync knows {}",
                target.kind,
                known.join(", ")
            )
        })?;
    Ok((kind.open)(paths, name, target))
}
