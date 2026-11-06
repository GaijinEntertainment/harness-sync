use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::item::Source;

#[derive(Debug)]
pub(crate) struct Paths {
    pub(crate) home: PathBuf,
    pub(crate) claude_dir: PathBuf,
    pub(crate) state_dir: PathBuf,
}

impl Paths {
    pub(crate) fn from_env() -> Result<Self> {
        let home =
            std::env::home_dir().context("cannot determine the home directory of this user")?;
        let claude_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|dir| !dir.is_empty())
            .map_or_else(|| home.join(".claude"), PathBuf::from);
        Ok(Self {
            claude_dir,
            state_dir: home.join(".cc-sync"),
            home,
        })
    }

    pub(crate) fn json_file(&self, source: Source) -> PathBuf {
        match source {
            Source::Settings => self.claude_dir.join("settings.json"),
        }
    }

    pub(crate) fn config_file(&self) -> PathBuf {
        self.state_dir.join("config.toml")
    }

    pub(crate) fn backups_dir(&self) -> PathBuf {
        self.state_dir.join("backups")
    }

    pub(crate) fn missing_secrets_file(&self) -> PathBuf {
        self.state_dir.join("missing-secrets.toml")
    }

    pub(crate) fn repo_dir(&self) -> PathBuf {
        self.state_dir.join("repo.git")
    }
}
