//! The `target` commands, which change the targets of this machine.

use anyhow::{Result, ensure};

use crate::config::{Config, Target};
use crate::paths::Paths;
use crate::report::{Report, TargetReport, short_commit};
use crate::storage;

/// Adds `target` under `name`.
///
/// # Errors
///
/// Fails when the URL of `target` holds a password, another target has the name `name`, the storage kind is unknown,
/// or the target cannot be fetched.
pub fn add(paths: &Paths, name: &str, target: Target) -> Result<Report> {
    target.ensure_no_password()?;
    let config_file = paths.config_file();
    let mut config = Config::load_connected(&config_file)?;
    if let Some(existing) = config.targets.get(name) {
        ensure!(
            *existing == target,
            "the target `{name}` exists with {} (branch {}); remove it first to change it",
            existing.url,
            existing.branch
        );
        return Ok(Report::new(
            "target add",
            format!("the target `{name}` exists"),
        ));
    }
    let storage = storage::open_target(paths, name, &target)?;
    storage.connect()?;
    if let Err(err) = storage.fetch() {
        storage.disconnect()?;
        return Err(err);
    }
    let head = storage.head()?;
    let summary = head.as_deref().map_or_else(
        || {
            format!(
                "added the target `{name}`; branch {} is empty",
                target.branch
            )
        },
        |commit| {
            format!(
                "added the target `{name}`; branch {} is at {}",
                target.branch,
                short_commit(commit)
            )
        },
    );
    config.targets.insert(name.to_owned(), target);
    config.save(&config_file)?;
    Ok(Report::new("target add", summary))
}

/// Removes the target `name`, which is not the primary target.
///
/// # Errors
///
/// Fails when `name` is the primary target, or the sync configuration cannot be read or written.
pub fn remove(paths: &Paths, name: &str) -> Result<Report> {
    let config_file = paths.config_file();
    let mut config = Config::load_connected(&config_file)?;
    ensure!(
        config.primary != name,
        "`{name}` is the primary target; make another target primary first"
    );
    let Some(removed) = config.targets.remove(name) else {
        return Ok(Report::new(
            "target remove",
            format!("there is no target `{name}`"),
        ));
    };
    config.save(&config_file)?;
    storage::open_target(paths, name, &removed)?.disconnect()?;
    Ok(Report::new(
        "target remove",
        format!("removed the target `{name}`"),
    ))
}

/// Makes the target `name` the primary target, and drops the base commit.
///
/// # Errors
///
/// Fails when there is no target `name`, or the sync configuration cannot be read or written.
pub fn set_primary(paths: &Paths, name: &str) -> Result<Report> {
    let config_file = paths.config_file();
    let mut config = Config::load_connected(&config_file)?;
    ensure!(
        config.targets.contains_key(name),
        "there is no target `{name}`; add it with `harness-sync target add`"
    );
    if config.primary == name {
        return Ok(Report::new(
            "target primary",
            format!("`{name}` is the primary target"),
        ));
    }
    storage::open(paths, &config, &config.primary)?.clear_base()?;
    name.clone_into(&mut config.primary);
    config.harnesses_without_base.clear();
    config.save(&config_file)?;
    Ok(Report::new(
        "target primary",
        format!(
            "`{name}` is the primary target; this machine has no base commit for it, so run `harness-sync pull` and \
             settle each item that differs with --prefer or --take"
        ),
    ))
}

/// Lists the targets of this machine.
///
/// # Errors
///
/// Fails when the sync configuration cannot be read.
pub fn list(paths: &Paths) -> Result<Report> {
    let config = Config::load_connected(&paths.config_file())?;
    let targets = config
        .targets
        .iter()
        .map(|(name, target)| TargetReport {
            name: name.clone(),
            url: target.url.clone(),
            branch: target.branch.clone(),
            kind: target.kind.clone(),
            primary: *name == config.primary,
        })
        .collect();
    Ok(Report {
        targets,
        ..Report::new("target list", format!("primary target: {}", config.primary))
    })
}
