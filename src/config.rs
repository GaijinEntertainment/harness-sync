use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::atomic;
use crate::harness::Selection;
use crate::item::ItemId;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Target {
    pub(crate) url: String,
    pub(crate) branch: String,
    #[serde(default)]
    pub(crate) kind: Kind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    #[default]
    Git,
    Gerrit,
}

impl Kind {
    pub(crate) const fn from_gerrit_flag(gerrit: bool) -> Self {
        if gerrit { Self::Gerrit } else { Self::Git }
    }
}

pub(crate) const ALL_TARGETS: &str = "all";

pub(crate) fn parse_target_name(text: &str) -> Result<String> {
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
        toml::from_str(&text)
            .map(Some)
            .with_context(|| format!("cannot parse {}", path.display()))
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
