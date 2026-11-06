use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug)]
pub(crate) struct Paths {
    pub(crate) state_dir: PathBuf,
}

impl Paths {
    pub(crate) fn from_env() -> Result<Self> {
        let home =
            std::env::home_dir().context("cannot determine the home directory of this user")?;
        Ok(Self {
            state_dir: home.join(".cc-sync"),
        })
    }

    pub(crate) fn config_file(&self) -> PathBuf {
        self.state_dir.join("config.toml")
    }

    pub(crate) fn repo_dir(&self) -> PathBuf {
        self.state_dir.join("repo.git")
    }
}
