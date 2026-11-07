use std::collections::BTreeSet;

use anyhow::{Context, Result};

use crate::compare::ThreeWay;
use crate::compare::{Difference, ItemState};
use crate::config::Config;
use crate::declarations::Declarations;
use crate::git::{self, BASE_REF, Git};
use crate::item::{ItemId, Items, KeyPath};
use crate::paths::Paths;
use crate::report::UndeclaredMarketplace;
use crate::{files, home, marketplaces, secret, setup};

#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) config: Config,
    pub(crate) git: Git,
    pub(crate) base_commit: Option<String>,
    pub(crate) remote_commit: Option<String>,
    pub(crate) pins: BTreeSet<ItemId>,
    pub(crate) declarations: Declarations,
    pub(crate) local_setup: Items,
    pub(crate) undeclared_marketplaces: Vec<UndeclaredMarketplace>,
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
        let pins = config.pinned_items()?;
        let declarations = Declarations::load(&git, remote_commit.as_deref())?;
        let local_setup = setup::load_local(paths, &git, &declarations)?;
        let undeclared_marketplaces = marketplaces::undeclared(paths, &local_setup, &declarations)?;
        let missing_secrets = secret::load_missing(&paths.missing_secrets_file())?;
        let mut local = secret::local_view(&local_setup, &missing_secrets, &declarations.secrets);
        for value in local.values_mut() {
            home::tokenize(value, &paths.home);
        }
        let base = setup::load_snapshot(&git, base_commit.as_deref(), &declarations)?;
        let remote = setup::load_snapshot(&git, remote_commit.as_deref(), &declarations)?;
        if cfg!(not(unix)) {
            files::carry_executable(&mut local, &[&base, &remote]);
        }
        let three_way = ThreeWay {
            base,
            local,
            remote,
        };
        Ok(Self {
            config,
            git,
            base_commit,
            remote_commit,
            pins,
            declarations,
            local_setup,
            undeclared_marketplaces,
            missing_secrets,
            three_way,
        })
    }

    pub(crate) fn differences(&self) -> Vec<Difference<'_>> {
        let mut differences = self.three_way.differences();
        for difference in &mut differences {
            if self.pins.contains(difference.id) {
                difference.state = ItemState::Pinned;
            }
        }
        differences
    }

    pub(crate) fn outgoing(&self) -> Items {
        let mut items = self.three_way.local.clone();
        for id in &self.pins {
            match self.three_way.remote.get(id) {
                Some(value) => items.insert(id.clone(), value.clone()),
                None => items.remove(id),
            };
        }
        items
    }
}
