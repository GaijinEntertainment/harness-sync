use anyhow::{Context, Result};

use crate::compare::ThreeWay;
use crate::config::Config;
use crate::git::{self, BASE_REF, Git};
use crate::paths::Paths;
use crate::setup;

#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) config: Config,
    pub(crate) git: Git,
    pub(crate) base_commit: Option<String>,
    pub(crate) remote_commit: Option<String>,
    pub(crate) three_way: ThreeWay,
}

impl Session {
    pub(crate) fn open(paths: &Paths) -> Result<Self> {
        let config = Config::load(&paths.config_file())?.context(
            "this machine is not connected to a remote; run `cc-sync init <remote>` first",
        )?;
        let git = Git::open(paths.repo_dir());
        git.fetch()?;
        let base_commit = git.resolve(BASE_REF)?;
        let remote_commit = git.resolve(&git::remote_ref(&config.branch))?;
        let three_way = ThreeWay {
            base: setup::load_snapshot(&git, base_commit.as_deref())?,
            local: setup::load_local(paths)?,
            remote: setup::load_snapshot(&git, remote_commit.as_deref())?,
        };
        Ok(Self {
            config,
            git,
            base_commit,
            remote_commit,
            three_way,
        })
    }
}
