use anyhow::{Context, Result};
use serde::Deserialize;

use crate::item::{ItemId, KeyPath};
use crate::storage::Storage;

pub(crate) const DECLARATIONS_FILE: &str = "harness-sync.toml";

#[derive(Debug, Default)]
pub(crate) struct Declarations {
    pub(crate) secrets: Vec<KeyPath>,
    pub(crate) include: Vec<KeyPath>,
    exclude: Vec<KeyPath>,
}

#[derive(Debug, Deserialize)]
struct DeclarationsFile {
    #[serde(default)]
    secrets: Vec<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

impl Declarations {
    pub(crate) fn load(storage: &dyn Storage, commit: Option<&str>) -> Result<Self> {
        let Some(commit) = commit else {
            return Ok(Self::default());
        };
        let Some(bytes) = storage
            .read_files(commit, &[DECLARATIONS_FILE])?
            .into_iter()
            .next()
            .flatten()
        else {
            return Ok(Self::default());
        };
        let context = || format!("cannot parse {DECLARATIONS_FILE} of the remote snapshot");
        let text = String::from_utf8(bytes).with_context(context)?;
        Self::parse(&text).with_context(context)
    }

    pub(crate) fn parse(text: &str) -> Result<Self> {
        let file: DeclarationsFile = toml::from_str(text)?;
        let parse = |list: &[String]| -> Result<Vec<KeyPath>> {
            list.iter().map(|key| key.parse()).collect()
        };
        Ok(Self {
            secrets: parse(&file.secrets)?,
            include: parse(&file.include)?,
            exclude: parse(&file.exclude)?,
        })
    }

    pub(crate) fn excludes(&self, id: &ItemId) -> bool {
        self.exclude
            .iter()
            .any(|path| path.covers(&id.file, &id.path))
    }
}
