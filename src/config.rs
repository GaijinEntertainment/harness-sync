use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::atomic;
use crate::item::ItemId;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Config {
    pub(crate) primary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) pins: Vec<String>,
    pub(crate) targets: BTreeMap<String, Target>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Target {
    pub(crate) url: String,
    pub(crate) branch: String,
}

impl Config {
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
