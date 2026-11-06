use anyhow::{Context, Result, ensure};

use crate::config::Config;
use crate::item::ItemId;
use crate::paths::Paths;
use crate::report::{Outcome, Report};

pub(crate) fn run(paths: &Paths, id: &ItemId, pinned: bool) -> Result<Report> {
    ensure!(
        id.names_an_item(),
        "`{id}` is not an item id: pin a key of settings.json, or one entry of a key that holds entries, such as `settings:/env/NAME`"
    );
    let config_file = paths.config_file();
    let mut config = Config::load(&config_file)?
        .context("this machine is not connected to a remote; run `cc-sync init <remote>` first")?;
    let text = id.to_string();
    let changed = if pinned {
        let added = !config.pins.contains(&text);
        if added {
            config.pins.push(text);
            config.pins.sort();
        }
        added
    } else {
        let before = config.pins.len();
        config.pins.retain(|pin| pin != &text);
        config.pins.len() != before
    };
    if changed {
        config.save(&config_file)?;
    }
    let summary = match (pinned, changed) {
        (true, true) => format!("pinned {id}: push and pull leave it as it is on this machine"),
        (true, false) => format!("{id} is already pinned"),
        (false, true) => format!("unpinned {id}: push and pull move it again"),
        (false, false) => format!("{id} is not pinned"),
    };
    Ok(Report {
        command: if pinned { "pin" } else { "unpin" },
        outcome: Outcome::Ok,
        base_commit: None,
        remote_commit: None,
        items: Vec::new(),
        secrets_found: Vec::new(),
        missing_executables: Vec::new(),
        summary,
    })
}
