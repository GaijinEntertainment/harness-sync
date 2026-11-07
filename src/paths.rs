use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::item::JsonFile;

#[derive(Debug)]
pub(crate) struct Paths {
    pub(crate) home: PathBuf,
    pub(crate) claude_dir: PathBuf,
    claude_json: PathBuf,
    pub(crate) state_dir: PathBuf,
}

impl Paths {
    pub(crate) fn from_env() -> Result<Self> {
        let home =
            std::env::home_dir().context("cannot determine the home directory of this user")?;
        let config_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from);
        Ok(Self::new(home, config_dir))
    }

    #[cfg(test)]
    pub(crate) fn for_home(home: &std::path::Path) -> Self {
        Self::new(home.to_path_buf(), None)
    }

    fn new(home: PathBuf, config_dir: Option<PathBuf>) -> Self {
        let claude_json = config_dir
            .as_ref()
            .map_or_else(|| home.join(".claude.json"), |dir| dir.join(".claude.json"));
        let claude_dir = config_dir.unwrap_or_else(|| home.join(".claude"));
        Self {
            claude_json,
            claude_dir,
            state_dir: home.join(".harness-sync"),
            home,
        }
    }

    pub(crate) fn json_file(&self, file: JsonFile) -> PathBuf {
        match file {
            JsonFile::Settings => self.claude_dir.join("settings.json"),
            JsonFile::Claude => self.claude_json.clone(),
        }
    }

    pub(crate) fn config_file(&self) -> PathBuf {
        self.state_dir.join("config.toml")
    }

    pub(crate) fn backups_dir(&self) -> PathBuf {
        self.state_dir.join("backups")
    }

    pub(crate) fn known_marketplaces_file(&self) -> PathBuf {
        self.claude_dir
            .join("plugins")
            .join("known_marketplaces.json")
    }

    pub(crate) fn missing_secrets_file(&self) -> PathBuf {
        self.state_dir.join("missing-secrets.toml")
    }

    pub(crate) fn repo_dir(&self) -> PathBuf {
        self.state_dir.join("repo.git")
    }
}
