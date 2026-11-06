use std::collections::BTreeSet;

use anyhow::{Context, Result};

use crate::compare::ThreeWay;
use crate::config::Config;
use crate::declarations::Declarations;
use crate::git::{self, BASE_REF, Git};
use crate::item::{Items, KeyPath};
use crate::paths::Paths;
use crate::{home, secret, setup};

#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) config: Config,
    pub(crate) git: Git,
    pub(crate) base_commit: Option<String>,
    pub(crate) remote_commit: Option<String>,
    pub(crate) local_setup: Items,
    pub(crate) missing_secrets: BTreeSet<KeyPath>,
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
        let local_setup = setup::load_local(paths)?;
        let missing_secrets = secret::load_missing(&paths.missing_secrets_file())?;
        let declarations = Declarations::load(&git, remote_commit.as_deref())?;
        let mut local = secret::local_view(&local_setup, &missing_secrets, &declarations.secrets);
        for value in local.values_mut() {
            home::tokenize(value, &paths.home);
        }
        let three_way = ThreeWay {
            base: setup::load_snapshot(&git, base_commit.as_deref())?,
            local,
            remote: setup::load_snapshot(&git, remote_commit.as_deref())?,
        };
        Ok(Self {
            config,
            git,
            base_commit,
            remote_commit,
            local_setup,
            missing_secrets,
            three_way,
        })
    }
}
