use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug)]
pub(crate) struct Paths {
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
        })
    }

    pub(crate) fn settings_file(&self) -> PathBuf {
        self.claude_dir.join("settings.json")
    }

    pub(crate) fn config_file(&self) -> PathBuf {
        self.state_dir.join("config.toml")
    }

    pub(crate) fn backups_dir(&self) -> PathBuf {
        self.state_dir.join("backups")
    }

    pub(crate) fn repo_dir(&self) -> PathBuf {
        self.state_dir.join("repo.git")
    }
}
