use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};

use crate::git::{Git, SnapshotFile};
use crate::item::{Items, settings_document, settings_items};
use crate::paths::Paths;

pub(crate) const SNAPSHOT_SETTINGS_FILE: &str = "settings.json";

pub(crate) fn load_local(paths: &Paths) -> Result<Items> {
    let path = paths.settings_file();
    match fs::read(&path) {
        Ok(bytes) => parse_settings(&bytes, &path),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Items::new()),
        Err(err) => Err(err).with_context(|| format!("cannot read {}", path.display())),
    }
}

pub(crate) fn load_snapshot(git: &Git, commit: Option<&str>) -> Result<Items> {
    let Some(commit) = commit else {
        return Ok(Items::new());
    };
    let spec = format!("{commit}:{SNAPSHOT_SETTINGS_FILE}");
    let settings = git
        .read_files(std::slice::from_ref(&spec))?
        .into_iter()
        .next()
        .flatten();
    settings.map_or_else(
        || Ok(Items::new()),
        |bytes| parse_settings(&bytes, Path::new(&spec)),
    )
}

fn parse_settings(bytes: &[u8], origin: &Path) -> Result<Items> {
    let document = serde_json::from_slice(bytes)
        .with_context(|| format!("{} is not valid JSON", origin.display()))?;
    settings_items(document)
        .with_context(|| format!("cannot read the items of {}", origin.display()))
}

pub(crate) fn snapshot_files(items: &Items) -> Result<Vec<SnapshotFile>> {
    let mut content = serde_json::to_vec_pretty(&settings_document(items))
        .context("cannot serialize the settings of the remote snapshot")?;
    content.push(b'\n');
    Ok(vec![SnapshotFile {
        path: SNAPSHOT_SETTINGS_FILE.to_owned(),
        content,
    }])
}
