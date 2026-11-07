use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

const CLAUDE_DIR: &str = ".claude";
const CLAUDE_JSON: &str = ".claude.json";

#[derive(Debug)]
pub(crate) struct Paths {
    pub(crate) home: PathBuf,
    pub(crate) claude_dir: PathBuf,
    roots: Vec<(&'static str, PathBuf)>,
    pub(crate) state_dir: PathBuf,
}

pub(crate) fn is_harness_path(file: &str) -> bool {
    [CLAUDE_JSON, CLAUDE_DIR].iter().any(|root| {
        file == *root
            || file
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('/'))
    })
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
    pub(crate) fn for_home(home: &Path) -> Self {
        Self::new(home.to_path_buf(), None)
    }

    fn new(home: PathBuf, config_dir: Option<PathBuf>) -> Self {
        let claude_json = config_dir
            .as_ref()
            .map_or_else(|| home.join(CLAUDE_JSON), |dir| dir.join(CLAUDE_JSON));
        let claude_dir = config_dir.unwrap_or_else(|| home.join(CLAUDE_DIR));
        Self {
            roots: vec![(CLAUDE_JSON, claude_json), (CLAUDE_DIR, claude_dir.clone())],
            claude_dir,
            state_dir: home.join(".harness-sync"),
            home,
        }
    }

    pub(crate) fn local(&self, file: &str) -> PathBuf {
        for (root, dir) in &self.roots {
            if file == *root {
                return dir.clone();
            }
            if let Some(rest) = file
                .strip_prefix(root)
                .and_then(|rest| rest.strip_prefix('/'))
            {
                return dir.join(rest);
            }
        }
        self.home.join(file)
    }

    pub(crate) fn home_path(&self, local: &Path) -> Option<String> {
        let (root, relative) = self
            .roots
            .iter()
            .find_map(|(root, dir)| Some((*root, local.strip_prefix(dir).ok()?)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_path_maps_to_the_claude_config_dir() {
        let home = Path::new("/home/me");
        let paths = Paths::new(home.to_path_buf(), Some(PathBuf::from("/cfg")));

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

    #[test]
    fn harness_paths_are_the_claude_directory_and_claude_json() {
        assert!(is_harness_path(".claude.json"));
        assert!(is_harness_path(".claude/settings.json"));
        assert!(!is_harness_path(".claudex/a"));
        assert!(!is_harness_path("README.md"));
    }
}
