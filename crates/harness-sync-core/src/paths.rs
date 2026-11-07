//! The places on this machine: the home directory, the directory of each harness root, and the state directory of
//! harness-sync.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

use crate::harness::{self, Root};
use crate::item::is_within;

/// The places on this machine.
#[derive(Debug)]
pub struct Paths {
    pub(crate) home: PathBuf,
    roots: Vec<(&'static str, PathBuf)>,
    pub(crate) state_dir: PathBuf,
}

impl Paths {
    /// The places of the current user, with each harness root moved by its environment variable.
    ///
    /// # Errors
    ///
    /// Fails when the home directory of the user is unknown.
    pub fn from_env() -> Result<Self> {
        let home =
            std::env::home_dir().context("cannot determine the home directory of this user")?;
        Ok(Self::new(home, |name| {
            std::env::var_os(name)
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from)
        }))
    }

    #[cfg(test)]
    pub(crate) fn for_home(home: &Path) -> Self {
        Self::new(home.to_path_buf(), |_| None)
    }

    fn new(home: PathBuf, env: impl Fn(&str) -> Option<PathBuf>) -> Self {
        let roots = harness::all()
            .iter()
            .flat_map(|harness| harness.roots)
            .map(|root| (root.path, root_dir(root, &home, &env)))
            .collect();
        Self {
            roots,
            state_dir: home.join(".harness-sync"),
            home,
        }
    }

    /// The location on this machine of `file`, a path relative to the home directory.
    #[must_use]
    pub fn local(&self, file: &str) -> PathBuf {
        let Some((root, dir)) = self.roots.iter().find(|(root, _)| is_within(file, root)) else {
            return self.home.join(file);
        };
        file[root.len()..]
            .strip_prefix('/')
            .map_or_else(|| dir.clone(), |rest| dir.join(rest))
    }

    pub(crate) fn home_path(&self, local: &Path) -> Option<String> {
        let (root, relative) = self
            .roots
            .iter()
            .filter_map(|(root, dir)| Some((*root, dir, local.strip_prefix(dir).ok()?)))
            .max_by_key(|(_, dir, _)| dir.components().count())
            .map(|(root, _, relative)| (root, relative))
            .or_else(|| Some(("", local.strip_prefix(&self.home).ok()?)))?;
        let mut parts: Vec<&str> = if root.is_empty() {
            Vec::new()
        } else {
            vec![root]
        };
        for component in relative.components() {
            let Component::Normal(part) = component else {
                return None;
            };
            parts.push(part.to_str()?);
        }
        (!parts.is_empty()).then(|| parts.join("/"))
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

    /// The directory where a storage kind keeps its local state, such as a bare git repo.
    #[must_use]
    pub fn repo_dir(&self) -> PathBuf {
        self.state_dir.join("repo.git")
    }
}

fn root_dir(root: &Root, home: &Path, env: &impl Fn(&str) -> Option<PathBuf>) -> PathBuf {
    match root.env.and_then(env) {
        Some(dir) if root.in_env_dir.is_empty() => dir,
        Some(dir) => dir.join(root.in_env_dir),
        None => home.join(root.path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_path_maps_to_the_claude_config_dir() {
        let home = Path::new("/home/me");
        let paths = Paths::new(home.to_path_buf(), |name| {
            (name == "CLAUDE_CONFIG_DIR").then(|| PathBuf::from("/cfg"))
        });

        assert_eq!(
            paths.local(".claude/rules/a.md"),
            Path::new("/cfg/rules/a.md")
        );
        assert_eq!(paths.local(".claude.json"), Path::new("/cfg/.claude.json"));
        assert_eq!(paths.local(".claudex/a"), home.join(".claudex/a"));
        assert_eq!(
            paths.home_path(Path::new("/cfg/.claude.json")).as_deref(),
            Some(".claude.json")
        );
        assert_eq!(
            paths.home_path(Path::new("/cfg/hooks/x.sh")).as_deref(),
            Some(".claude/hooks/x.sh")
        );
        assert_eq!(
            paths.home_path(&home.join(".agents/a.md")).as_deref(),
            Some(".agents/a.md")
        );
        assert_eq!(paths.home_path(Path::new("/etc/x")), None);
    }
}
