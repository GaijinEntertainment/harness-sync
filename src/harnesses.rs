use anyhow::Result;

use crate::config::Config;
use crate::harness::{self, HARNESSES};
use crate::paths::Paths;
use crate::report::{HarnessReport, Report};

pub(crate) fn add(paths: &Paths, name: &str) -> Result<Report> {
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
    config.save(&config_file)?;
    Ok(Report::new(
        "harness add",
        format!("this machine syncs {name}; run `harness-sync pull` to take its items"),
    ))
}

pub(crate) fn remove(paths: &Paths, name: &str) -> Result<Report> {
    let config_file = paths.config_file();
    let mut config = Config::load_connected(&config_file)?;
    let before = config.harnesses.len();
    config.harnesses.retain(|synced| synced != name);
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

pub(crate) fn list(paths: &Paths) -> Result<Report> {
    let config = Config::load_connected(&paths.config_file())?;
    let harnesses = HARNESSES
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
