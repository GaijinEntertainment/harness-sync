//! The places on this machine: the home directory, the directory of each harness root, and the state directory of
//! harness-sync.

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::harness::{self, Root};
use crate::item::is_within;

pub(crate) const STATE_DIR: &str = ".harness-sync";

/// The places on this machine.
#[derive(Debug)]
pub struct Paths {
    pub(crate) home: PathBuf,
    roots: Vec<RootDir>,
    pub(crate) state_dir: PathBuf,
}

#[derive(Debug)]
struct RootDir {
    path: &'static str,
    dir: PathBuf,
    home_or_env_dir: PathBuf,
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
            .map(|root| {
                let env_dir = root.env.and_then(&env);
                RootDir {
                    path: root.path,
                    dir: root_dir(root, &home, env_dir.as_deref()),
                    home_or_env_dir: env_dir.unwrap_or_else(|| home.clone()),
                }
            })
            .collect();
        Self {
            roots,
            state_dir: home.join(STATE_DIR),
            home,
        }
    }

    /// The location on this machine of `file`, a path relative to the home directory.
    #[must_use]
    pub fn local(&self, file: &str) -> PathBuf {
        let Some(root) = self.root_of(file) else {
            return self.home.join(file);
        };
        file[root.path.len()..]
            .strip_prefix('/')
            .map_or_else(|| root.dir.clone(), |rest| root.dir.join(rest))
    }

    pub(crate) fn is_behind_symlink(&self, file: &str) -> bool {
        let top = self
            .root_of(file)
            .map_or(&self.home, |root| &root.home_or_env_dir);
        self.local(file)
            .ancestors()
            .take_while(|path| path != top)
            .any(|path| fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink()))
    }

    pub(crate) fn ensure_no_symlink(&self, file: &str) -> Result<()> {
        ensure!(
            !self.is_behind_symlink(file),
            "{} is a symlink or lies in a symlinked directory, and harness-sync never writes through a symlink",
            self.local(file).display()
        );
        Ok(())
    }

    pub(crate) fn remove_empty_dirs_above(&self, file: &str) {
        let top = self.root_of(file).map_or(&self.home, |root| &root.dir);
        let above = self.local(file);
        for dir in above.ancestors().skip(1).take_while(|dir| dir != top) {
            // ignored: a directory that still holds a file, or that cannot be removed, stays
            if fs::remove_dir(dir).is_err() {
                break;
            }
        }
    }

    fn root_of(&self, file: &str) -> Option<&RootDir> {
        self.roots.iter().find(|root| is_within(file, root.path))
    }

    pub(crate) fn home_path(&self, local: &Path) -> Option<String> {
        let (root, relative) = self
            .roots
            .iter()
            .filter_map(|root| Some((root.path, &root.dir, local.strip_prefix(&root.dir).ok()?)))
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

fn root_dir(root: &Root, home: &Path, env_dir: Option<&Path>) -> PathBuf {
    match env_dir {
        Some(dir) if root.in_env_dir.is_empty() => dir.to_path_buf(),
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

    #[cfg(unix)]
    #[test]
    fn a_symlink_below_the_home_or_the_moved_root_hides_the_files_behind_it() {
        use std::os::unix::fs::symlink;

        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        fs::create_dir_all(home.join("dotfiles/skills")).unwrap();
        fs::create_dir_all(home.join(".claude/rules")).unwrap();
        symlink(home.join("dotfiles/skills"), home.join(".claude/skills")).unwrap();
        symlink(home.join("dotfiles"), home.join("moved")).unwrap();
        let paths = Paths::for_home(home);
        let moved = Paths::new(home.to_path_buf(), |name| {
            (name == "CLAUDE_CONFIG_DIR").then(|| home.join("moved"))
        });

        assert!(paths.is_behind_symlink(".claude/skills"));
        assert!(paths.is_behind_symlink(".claude/skills/x/SKILL.md"));
        assert!(!paths.is_behind_symlink(".claude/rules/a.md"));
        assert!(!moved.is_behind_symlink(".claude/skills/x/SKILL.md"));
        assert!(moved.is_behind_symlink("moved/x"));
    }
}
