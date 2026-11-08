//! The `secret set` command.

use anyhow::{Result, bail, ensure};
use serde_json::Value;

use crate::config::Config;
use crate::declarations::Declarations;
use crate::harness::keyed_file;
use crate::item::KeyPath;
use crate::json_edit::Edit;
use crate::paths::Paths;
use crate::report::Report;
use crate::setup::Backup;
use crate::{secret, setup, storage};

/// Sets the secret at `key` on this machine. `value` reads the secret value, and runs only when `key` is a secret key
/// path.
///
/// # Errors
///
/// Fails when `key` is not a secret key path of a keyed JSON file, `value` fails, or the file cannot be written.
pub fn run(paths: &Paths, key: &KeyPath, value: impl FnOnce() -> Result<String>) -> Result<Report> {
    let mut missing = secret::load_missing(&paths.missing_secrets_file())?;
    ensure!(
        missing.contains(key)
            || secret::is_secret_path(key, &[])
            || secret::is_secret_path(key, &declared_secrets(paths)?),
        "`{key}` is not a secret key path: no secret name rule matches it, harness-sync.toml in the repo does not declare it, \
         and pull did not record it as missing"
    );
    let value = value()?;
    if keyed_file(&key.file).is_none() || key.path.is_empty() {
        bail!("`{key}` is not a key path in a JSON file that harness-sync splits into keys");
    }
    setup::write_local(
        paths,
        &Backup::new(paths),
        &key.file,
        &[Edit {
            path: &key.path,
            value: Some(&Value::String(value)),
        }],
    )?;
    if missing.remove(key) {
        secret::save_missing(&paths.missing_secrets_file(), &missing)?;
    }
    Ok(Report::new(
        "secret set",
        format!(
            "set the secret {key} in {}",
            paths.local(&key.file).display()
        ),
    ))
}

fn declared_secrets(paths: &Paths) -> Result<Vec<KeyPath>> {
    let Some(config) = Config::load(&paths.config_file())? else {
        return Ok(Vec::new());
    };
    let storage = storage::open(paths, &config, &config.primary)?;
    storage.fetch()?;
    let remote_commit = storage.head()?;
    Ok(Declarations::load(storage.as_ref(), remote_commit.as_deref())?.secrets)
}
