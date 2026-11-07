mod claude_code;

use anyhow::Result;
use serde_json::Value;

use crate::declarations::Declarations;
use crate::item::{ItemId, Items, KeyPath, is_within};
use crate::paths::Paths;
use crate::report::UndeclaredMarketplace;

pub(crate) static HARNESSES: [&Harness; 1] = [&claude_code::HARNESS];

pub(crate) type Commands = fn(&ItemId, &Value) -> Vec<(KeyPath, String)>;
pub(crate) type MarketplaceCheck =
    fn(&Paths, &Items, &Declarations) -> Result<Vec<UndeclaredMarketplace>>;

#[derive(Debug)]
pub(crate) struct Harness {
    pub(crate) roots: &'static [Root],
    pub(crate) keyed_files: &'static [KeyedFile],
    pub(crate) files: &'static [&'static str],
    pub(crate) dirs: &'static [&'static str],
    pub(crate) skipped_dirs: &'static [&'static str],
    pub(crate) never_referenced: &'static [&'static str],
    pub(crate) credentials: &'static [&'static str],
    pub(crate) commands: Commands,
    pub(crate) undeclared_marketplaces: MarketplaceCheck,
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
}

impl KeyedFile {
    pub(crate) fn holds(&self, key: &str) -> bool {
        match self.keys {
            Keys::All => true,
            Keys::Only(keys) => keys.contains(&key),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_harness_owns_the_paths_at_and_below_its_roots() {
        assert!(owner(".claude.json").is_some());
        assert!(owner(".claude/settings.json").is_some());
        assert!(owner(".claudex/a").is_none());
        assert!(owner("README.md").is_none());
    }
}
