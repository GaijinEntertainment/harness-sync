//! The sync configuration of this machine in `~/.harness-sync/config.toml`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::harness::Selection;
use crate::item::ItemId;
use crate::{atomic, storage};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Config {
    pub(crate) primary: String,
    #[serde(default)]
    pub(crate) harnesses: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) harnesses_without_base: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) pins: Vec<String>,
    pub(crate) targets: BTreeMap<String, Target>,
}

/// A place that push can send the snapshot to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    /// The URL or path of the remote.
    pub url: String,
    /// The branch that holds the remote snapshot.
    pub branch: String,
    /// The name of the storage kind of the target.
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    storage::DEFAULT_KIND.to_owned()
}

pub(crate) const ALL_TARGETS: &str = "all";

/// Checks that `text` can name a target.
///
/// # Errors
///
/// Fails for an empty name, a name with characters other than letters, digits, `-`, and `_`, and the name `all`.
pub fn parse_target_name(text: &str) -> Result<String> {
    ensure!(
        !text.is_empty()
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "`{text}` is not a target name; use letters, digits, `-`, and `_`"
    );
    ensure!(
        text != ALL_TARGETS,
        "`{ALL_TARGETS}` names every target and cannot name one"
    );
    Ok(text.to_owned())
}

impl Target {
    pub(crate) fn ensure_no_password(&self) -> Result<()> {
        let has_password = self
            .url
            .split_once("://")
            .and_then(|(_, rest)| rest.split('/').next())
            .and_then(|authority| authority.rsplit_once('@'))
            .is_some_and(|(user_info, _)| user_info.contains(':'));
        ensure!(
            !has_password,
            "the URL of the target holds a password, and harness-sync prints target URLs; keep the password in a git \
             credential helper"
        );
        Ok(())
    }
}

impl Config {
    pub(crate) fn load_connected(path: &Path) -> Result<Self> {
        Self::load(path)?.context(
            "this machine is not connected to a remote; run `harness-sync init <remote>` first",
        )
    }

    pub(crate) fn load(path: &Path) -> Result<Option<Self>> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
        };
        let config: Self =
            toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))?;
        for (name, target) in &config.targets {
            target
                .ensure_no_password()
                .with_context(|| format!("cannot use the target `{name}` of {}", path.display()))?;
        }
        Ok(Some(config))
    }

    pub(crate) fn primary(&self) -> Result<&Target> {
        self.targets.get(&self.primary).with_context(|| {
            format!(
                "the primary target `{}` is not among the targets of the sync configuration",
                self.primary
            )
        })
    }

    pub(crate) fn selection(&self) -> Result<Selection> {
        Selection::new(&self.harnesses)
    }

    pub(crate) fn pinned_items(&self) -> Result<BTreeSet<ItemId>> {
        self.pins
            .iter()
            .map(|id| id.parse())
            .collect::<Result<_>>()
            .context("cannot parse the pins of the sync configuration")
    }

    pub(crate) fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string(self).context("cannot serialize the sync configuration")?;
        atomic::write(path, text.as_bytes())
    }
}
