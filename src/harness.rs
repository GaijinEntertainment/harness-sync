mod claude_code;
mod pi;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::declarations::Declarations;
use crate::item::{ItemId, Items, KeyPath, is_within};
use crate::paths::Paths;
use crate::report::UndeclaredMarketplace;

pub(crate) static HARNESSES: [&Harness; 2] = [&claude_code::HARNESS, &pi::HARNESS];

pub(crate) type Commands = fn(&ItemId, &Value) -> Vec<(KeyPath, String)>;
pub(crate) type MarketplaceCheck =
    fn(&Paths, &Items, &Declarations) -> Result<Vec<UndeclaredMarketplace>>;

#[derive(Debug)]
pub(crate) struct Harness {
    pub(crate) name: &'static str,
    pub(crate) roots: &'static [Root],
    pub(crate) keyed_files: &'static [KeyedFile],
    pub(crate) files: &'static [&'static str],
    pub(crate) dirs: &'static [&'static str],
    pub(crate) skipped_dirs: &'static [&'static str],
    pub(crate) never_referenced: &'static [&'static str],
    pub(crate) credentials: &'static [&'static str],
    pub(crate) commands: Commands,
    pub(crate) undeclared_marketplaces: Option<MarketplaceCheck>,
}

#[derive(Debug)]
pub(crate) struct Root {
    pub(crate) path: &'static str,
    pub(crate) env: Option<&'static str>,
    pub(crate) in_env_dir: &'static str,
}

#[derive(Debug)]
pub(crate) struct KeyedFile {
    pub(crate) path: &'static str,
    pub(crate) keys: Keys,
    pub(crate) split: &'static [&'static str],
}

#[derive(Debug)]
pub(crate) enum Keys {
    All,
    Only(&'static [&'static str]),
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

pub(crate) fn keyed_files() -> impl Iterator<Item = &'static KeyedFile> {
    HARNESSES.iter().flat_map(|harness| harness.keyed_files)
}

pub(crate) fn keyed_file(path: &str) -> Option<&'static KeyedFile> {
    keyed_files().find(|keyed| keyed.path == path)
}

pub(crate) fn owner(file: &str) -> Option<&'static Harness> {
    HARNESSES
        .iter()
        .copied()
        .find(|harness| harness.roots.iter().any(|root| is_within(file, root.path)))
}

pub(crate) fn is_credentials(file: &str) -> bool {
    HARNESSES
        .iter()
        .any(|harness| harness.credentials.contains(&file))
}

pub(crate) fn is_skipped_dir(dir: &str) -> bool {
    HARNESSES
        .iter()
        .any(|harness| harness.skipped_dirs.contains(&dir))
}

pub(crate) fn by_name(name: &str) -> Result<&'static Harness> {
    HARNESSES
        .iter()
        .copied()
        .find(|harness| harness.name == name)
        .with_context(|| {
            let names: Vec<&str> = HARNESSES.iter().map(|harness| harness.name).collect();
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
