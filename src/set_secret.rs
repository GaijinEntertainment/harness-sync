use std::io::{self, IsTerminal};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

use crate::config::Config;
use crate::declarations::Declarations;
use crate::git::{self, Git};
use crate::item::{KeyPath, Source};
use crate::json_edit::Edit;
use crate::output;
use crate::paths::Paths;
use crate::report::{Outcome, Report};
use crate::setup::Backup;
use crate::{secret, setup};

pub(crate) fn run(paths: &Paths, key: &KeyPath, prompts: bool) -> Result<Report> {
    let mut missing = secret::load_missing(&paths.missing_secrets_file())?;
    ensure!(
        missing.contains(key) || secret::is_secret_path(key, &declared_secrets(paths)?),
        "`{key}` is not a secret key path: no secret name rule matches it, cc-sync.toml in the repo does not declare it, \
         and pull did not record it as missing"
    );
    let value = if prompts {
        output::ask_secret(key)?.context("no value entered")?
    } else {
        read_stdin()?
    };
    let Source::Json(file) = key.source else {
        bail!("`{key}` is not a key path in a JSON file");
    };
    setup::write_local(
        paths,
        &Backup::new(paths),
        file,
        &[Edit {
            path: &key.path,
            value: Some(&Value::String(value)),
        }],
    )?;
    if missing.remove(key) {
        secret::save_missing(&paths.missing_secrets_file(), &missing)?;
    }
    Ok(Report {
        command: "secret set",
        outcome: Outcome::Ok,
        base_commit: None,
        remote_commit: None,
        items: Vec::new(),
        secrets_found: Vec::new(),
        missing_executables: Vec::new(),
        undeclared_marketplaces: Vec::new(),
        summary: format!(
            "set the secret {key} in {}",
            paths.json_file(file).display()
        ),
    })
}

fn declared_secrets(paths: &Paths) -> Result<Vec<KeyPath>> {
    let Some(config) = Config::load(&paths.config_file())? else {
        return Ok(Vec::new());
    };
    let git = Git::open(paths.repo_dir());
    let remote_commit =
        git.resolve(&git::remote_ref(&config.primary, &config.primary()?.branch))?;
    Ok(Declarations::load(&git, remote_commit.as_deref())?.secrets)
}

fn read_stdin() -> Result<String> {
    if io::stdin().is_terminal() {
        bail!(
            "stdin is a terminal but prompts are off: pipe the value on stdin, or run without --no-input and with stdout on the terminal"
        );
    }
    let text =
        io::read_to_string(io::stdin()).context("cannot read the secret value from stdin")?;
    let value = text.strip_suffix('\n').map_or(text.as_str(), |line| {
        line.strip_suffix('\r').unwrap_or(line)
    });
    ensure!(!value.is_empty(), "stdin holds no secret value");
    Ok(value.to_owned())
}
