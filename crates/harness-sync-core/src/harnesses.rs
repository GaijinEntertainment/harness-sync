//! The `harness` commands, which change the harnesses that this machine syncs.

use anyhow::Result;

use crate::config::Config;
use crate::harness;
use crate::paths::Paths;
use crate::report::{HarnessReport, Report};
use crate::storage;

/// Makes this machine sync the harness `name`. Its items have no base until the next pull.
///
/// # Errors
///
/// Fails when the binary has no harness `name`, or the sync configuration cannot be read or written.
pub fn add(paths: &Paths, name: &str) -> Result<Report> {
    harness::by_name(name)?;
    let config_file = paths.config_file();
    let mut config = Config::load_connected(&config_file)?;
    if config.harnesses.iter().any(|synced| synced == name) {
        return Ok(Report::new(
            "harness add",
            format!("this machine syncs {name}"),
        ));
    }
    config.harnesses.push(name.to_owned());
    config.harnesses.sort();
    if storage::open(paths, &config, &config.primary)?
        .base()?
        .is_some()
    {
        config.harnesses_without_base.push(name.to_owned());
    }
    config.save(&config_file)?;
    Ok(Report::new(
        "harness add",
        format!("this machine syncs {name}; run `harness-sync pull` to take its items"),
    ))
}

/// Makes this machine stop syncing the harness `name`; its files stay as they are.
///
/// # Errors
///
/// Fails when the sync configuration cannot be read or written.
pub fn remove(paths: &Paths, name: &str) -> Result<Report> {
    let config_file = paths.config_file();
    let mut config = Config::load_connected(&config_file)?;
    let before = config.harnesses.len();
    config.harnesses.retain(|synced| synced != name);
    config.harnesses_without_base.retain(|added| added != name);
    if config.harnesses.len() == before {
        return Ok(Report::new(
            "harness remove",
            format!("this machine does not sync {name}"),
        ));
    }
    config.save(&config_file)?;
    Ok(Report::new(
        "harness remove",
        format!(
            "this machine stops syncing {name}; its files stay as they are on this machine and in the repo"
        ),
    ))
}

/// Lists the harnesses of the binary, and which of them this machine syncs.
///
/// # Errors
///
/// Fails when the sync configuration cannot be read.
pub fn list(paths: &Paths) -> Result<Report> {
    let config = Config::load_connected(&paths.config_file())?;
    let harnesses = harness::all()
        .iter()
        .map(|harness| HarnessReport {
            name: harness.name,
            synced: config.harnesses.iter().any(|name| name == harness.name),
        })
        .collect();
    let summary = if config.harnesses.is_empty() {
        "this machine syncs no harness; add one with `harness-sync harness add <name>`".to_owned()
    } else {
        format!("this machine syncs {}", config.harnesses.join(", "))
    };
    Ok(Report {
        harnesses,
        ..Report::new("harness list", summary)
    })
}
