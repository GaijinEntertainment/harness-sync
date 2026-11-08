//! The description of a harness: its catalog of supported items and the hooks that read its own formats. A harness
//! crate exports one [`Harness`] value, and a binary lists it in its [`Registry`](crate::registry::Registry).
//!
//! Every path in a description is relative to the home directory and uses `/`, such as `.claude/settings.json`.

use anyhow::{Context, Result};
use serde_json::Value;

use crate::declarations::Declarations;
use crate::item::{ItemId, Items, KeyPath, is_within, is_within_ignoring_case};
use crate::paths::Paths;
use crate::registry;
use crate::report::UndeclaredMarketplace;

/// Finds the commands that a synced item runs: each key path of a command string in `value`, with the command.
pub type Commands = fn(id: &ItemId, value: &Value) -> Vec<(KeyPath, Command)>;

/// A command that a synced item runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// A line that the harness runs through a shell, such as a hook.
    Shell(String),
    /// A program that the harness starts without a shell, such as the `command` of an MCP server.
    Program(String),
}

/// Finds the marketplaces that the harness records on this machine and that the local setup does not declare.
pub type MarketplaceCheck = fn(
    paths: &Paths,
    setup: &Items,
    declarations: &Declarations,
) -> Result<Vec<UndeclaredMarketplace>>;

/// One harness: where its setup lives, which of its files and keys sync, and what it never syncs.
#[derive(Debug)]
pub struct Harness {
    /// The name that the sync configuration and the `harness` commands use, such as `claude-code`.
    pub name: &'static str,
    /// The paths that the harness owns, with the files and directories below them.
    pub roots: &'static [Root],
    /// The JSON files that sync key by key.
    pub keyed_files: &'static [KeyedFile],
    /// The files that sync whole.
    pub files: &'static [&'static str],
    /// The directories whose files sync whole.
    pub dirs: &'static [&'static str],
    /// The directories below `dirs` whose files never sync.
    pub skipped_dirs: &'static [&'static str],
    /// The directories whose files never sync as files that a synced item points to.
    pub never_referenced: &'static [&'static str],
    /// The credentials files, which never sync and which `include` cannot add.
    pub credentials: &'static [&'static str],
    /// Finds the commands that the items of the harness run, for the check of missing executables after a pull.
    pub commands: Commands,
    /// Finds the marketplaces that do not sync because the settings do not declare them, when the harness has any.
    pub undeclared_marketplaces: Option<MarketplaceCheck>,
}

/// A path that a harness owns, and the environment variable that moves it.
#[derive(Debug)]
pub struct Root {
    /// The path below the home directory.
    pub path: &'static str,
    /// The environment variable that names the directory that holds the root in place of the home path.
    pub env: Option<&'static str>,
    /// The path of the root in the directory of `env`, or `""` when that directory is the root.
    pub in_env_dir: &'static str,
}

/// A JSON file that syncs as one item per top-level key.
#[derive(Debug)]
pub struct KeyedFile {
    /// The path of the file.
    pub path: &'static str,
    /// The top-level keys that sync.
    pub keys: Keys,
    /// The top-level keys that hold independent entries, each of which is one item.
    pub split: &'static [&'static str],
}

/// The top-level keys of a [`KeyedFile`] that sync. The repo's `include` list adds keys to them.
#[derive(Debug)]
pub enum Keys {
    /// Every key.
    All,
    /// The named keys.
    Only(&'static [&'static str]),
    /// Every key except the named ones.
    Except(&'static [&'static str]),
}

impl KeyedFile {
    pub(crate) fn holds(&self, key: &str) -> bool {
        match self.keys {
            Keys::All => true,
            Keys::Only(keys) => keys.contains(&key),
            Keys::Except(keys) => !keys.contains(&key),
        }
    }

    pub(crate) fn splits(&self, key: &str) -> bool {
        self.split.contains(&key)
    }

    pub(crate) fn item_length(&self, path: &[String]) -> usize {
        if path.first().is_some_and(|key| self.splits(key)) {
            2
        } else {
            1
        }
    }
}

pub(crate) fn all() -> &'static [&'static Harness] {
    registry::installed().harnesses
}

pub(crate) fn keyed_files() -> impl Iterator<Item = &'static KeyedFile> {
    all().iter().flat_map(|harness| harness.keyed_files)
}

pub(crate) fn keyed_file(path: &str) -> Option<&'static KeyedFile> {
    keyed_files().find(|keyed| keyed.path == path)
}

pub(crate) fn owner(file: &str) -> Option<&'static Harness> {
    all()
        .iter()
        .copied()
        .find(|harness| harness.roots.iter().any(|root| is_within(file, root.path)))
}

pub(crate) fn lists_as_never_whole(file: &str) -> bool {
    all().iter().any(|harness| {
        harness
            .keyed_files
            .iter()
            .map(|keyed| keyed.path)
            .chain(harness.credentials.iter().copied())
            .chain(harness.skipped_dirs.iter().copied())
            .any(|path| is_within_ignoring_case(file, path))
    })
}

pub(crate) fn by_name(name: &str) -> Result<&'static Harness> {
    all()
        .iter()
        .copied()
        .find(|harness| harness.name == name)
        .with_context(|| {
            let names: Vec<&str> = all().iter().map(|harness| harness.name).collect();
            format!(
                "there is no harness `{name}`; harness-sync syncs {}",
                names.join(", ")
            )
        })
}

impl Harness {
    pub(crate) fn is_present(&self, paths: &Paths) -> bool {
        self.roots
            .iter()
            .any(|root| paths.local(root.path).exists())
    }
}

#[derive(Debug)]
pub(crate) struct Selection {
    harnesses: Vec<&'static Harness>,
}

impl Selection {
    pub(crate) fn new(names: &[String]) -> Result<Self> {
        let harnesses = names
            .iter()
            .map(|name| by_name(name))
            .collect::<Result<_>>()
            .context("cannot read the harnesses of the sync configuration")?;
        Ok(Self { harnesses })
    }

    pub(crate) fn harnesses(&self) -> impl Iterator<Item = &'static Harness> + '_ {
        self.harnesses.iter().copied()
    }

    pub(crate) fn syncs(&self, file: &str) -> bool {
        owner(file).is_none() || self.owns(file)
    }

    pub(crate) fn owns(&self, file: &str) -> bool {
        owner(file).is_some_and(|owner| {
            self.harnesses
                .iter()
                .any(|harness| harness.name == owner.name)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_harness_owns_the_paths_at_and_below_its_roots() {
        let name = |file: &str| owner(file).map(|harness| harness.name);

        assert_eq!(name(".claude.json"), Some("claude-code"));
        assert_eq!(name(".claude/settings.json"), Some("claude-code"));
        assert_eq!(name(".pi/agent/models.json"), Some("pi"));
        assert_eq!(name(".claudex/a"), None);
        assert_eq!(name(".pi/other"), None);
        assert_eq!(name("README.md"), None);
    }
}
