use std::collections::BTreeSet;

use anyhow::Result;

use crate::compare::ThreeWay;
use crate::compare::{Difference, ItemState};
use crate::config::Config;
use crate::declarations::Declarations;
use crate::harness::Selection;
use crate::item::{ItemId, Items, KeyPath};
use crate::paths::Paths;
use crate::report::UndeclaredMarketplace;
use crate::storage::{self, Storage};
use crate::{files, home, secret, setup};

#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) config: Config,
    pub(crate) target: String,
    pub(crate) storage: Box<dyn Storage>,
    pub(crate) base_commit: Option<String>,
    pub(crate) remote_commit: Option<String>,
    pub(crate) pins: BTreeSet<ItemId>,
    pub(crate) selection: Selection,
    pub(crate) declarations: Declarations,
    pub(crate) local_setup: Items,
    pub(crate) undeclared_marketplaces: Vec<UndeclaredMarketplace>,
    pub(crate) missing_secrets: BTreeSet<KeyPath>,
    pub(crate) three_way: ThreeWay,
}

impl Session {
    pub(crate) fn open(paths: &Paths) -> Result<Self> {
        Self::open_target(paths, None)
    }

    pub(crate) fn open_target(paths: &Paths, target: Option<&str>) -> Result<Self> {
        let config = Config::load_connected(&paths.config_file())?;
        let target = target.unwrap_or(&config.primary).to_owned();
        let storage = storage::open(paths, &config, &target)?;
        storage.fetch()?;
        let remote_commit = storage.head()?;
        let primary = target == config.primary;
        let base_commit = if primary {
            storage.base()?
        } else {
            remote_commit.clone()
        };
        let pins = config.pinned_items()?;
        let selection = config.selection()?;
        let declarations = Declarations::load(storage.as_ref(), remote_commit.as_deref())?;
        let local_setup = setup::load_local(paths, storage.as_ref(), &declarations, &selection)?;
        let mut undeclared_marketplaces = Vec::new();
        for check in selection
            .harnesses()
            .filter_map(|harness| harness.undeclared_marketplaces)
        {
            undeclared_marketplaces.extend(check(paths, &local_setup, &declarations)?);
        }
        let mut remote = setup::load_snapshot(
            storage.as_ref(),
            remote_commit.as_deref(),
            &declarations,
            &selection,
        )?;
        remote.retain(|id, _| !paths.is_behind_symlink(&id.file));
        let base = if primary {
            let without_base = Selection::new(&config.harnesses_without_base)?;
            setup::load_snapshot(
                storage.as_ref(),
                base_commit.as_deref(),
                &declarations,
                &selection,
            )?
            .into_iter()
            .filter(|(id, _)| !without_base.owns(&id.file) && !paths.is_behind_symlink(&id.file))
            .collect()
        } else {
            remote.clone()
        };
        let mut missing_secrets = secret::load_missing(&paths.missing_secrets_file())?;
        let recorded = missing_secrets.len();
        missing_secrets.retain(|key| !secret::has_value(&local_setup, key));
        if missing_secrets.len() != recorded {
            secret::save_missing(&paths.missing_secrets_file(), &missing_secrets)?;
        }
        let mut local =
            secret::local_view(&local_setup, &missing_secrets, &declarations.secrets, &base);
        for value in local.values_mut() {
            home::tokenize(value, &paths.home);
        }
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
            target,
            storage,
            base_commit,
            remote_commit,
            pins,
            selection,
            declarations,
            local_setup,
            undeclared_marketplaces,
            missing_secrets,
            three_way,
        })
    }

    pub(crate) fn is_primary(&self) -> bool {
        self.target == self.config.primary
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
