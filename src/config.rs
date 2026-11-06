use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::atomic;
use crate::item::ItemId;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Config {
    pub(crate) remote: String,
    pub(crate) branch: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) pins: Vec<String>,
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
