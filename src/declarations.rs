use std::slice;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::git::Git;
use crate::item::{ItemId, KeyPath};

const DECLARATIONS_FILE: &str = "cc-sync.toml";

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
    pub(crate) fn load(git: &Git, commit: Option<&str>) -> Result<Self> {
        let Some(commit) = commit else {
            return Ok(Self::default());
        };
        let spec = format!("{commit}:{DECLARATIONS_FILE}");
        let Some(bytes) = git
            .read_files(slice::from_ref(&spec))?
            .into_iter()
            .next()
            .flatten()
        else {
            return Ok(Self::default());
        };
        let context = || format!("cannot parse {DECLARATIONS_FILE} of the remote snapshot");
        let text = String::from_utf8(bytes).with_context(context)?;
        let file: DeclarationsFile = toml::from_str(&text).with_context(context)?;
        let parse = |list: &[String]| -> Result<Vec<KeyPath>> {
            list.iter()
                .map(|key| key.parse())
                .collect::<Result<_>>()
                .with_context(context)
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
            .any(|path| path.source == id.source && id.path.starts_with(&path.path))
    }
}
