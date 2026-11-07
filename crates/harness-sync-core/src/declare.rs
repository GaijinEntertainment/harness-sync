//! The `declarations` commands, which show and change the lists of `harness-sync.toml` of a target.

use anyhow::{Context, Result, bail};
use toml::{Table, Value};

use crate::config::Config;
use crate::declarations::{DECLARATIONS_FILE, Declarations};
use crate::item::KeyPath;
use crate::paths::Paths;
use crate::report::{DeclarationsReport, Outcome, Report, Review, TargetPush, short_commit};
use crate::setup;
use crate::storage::{self, FileChange, Publish, Storage};

/// A list of `harness-sync.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    /// The key paths of declared secrets.
    Secrets,
    /// The files, directories, and keys that the catalogs leave out and that sync.
    Include,
    /// The items that do not sync.
    Exclude,
}

impl List {
    const fn key(self) -> &'static str {
        match self {
            Self::Secrets => "secrets",
            Self::Include => "include",
            Self::Exclude => "exclude",
        }
    }
}

struct Repo {
    storage: Box<dyn Storage>,
    target: String,
    primary: bool,
    head: Option<String>,
}

impl Repo {
    fn open(paths: &Paths, target: Option<&str>) -> Result<Self> {
        let config = Config::load_connected(&paths.config_file())?;
        let target = target.unwrap_or(&config.primary).to_owned();
        let storage = storage::open(paths, &config, &target)?;
        storage.fetch()?;
        let head = storage.head()?;
        Ok(Self {
            primary: target == config.primary,
            storage,
            target,
            head,
        })
    }

    fn table(&self) -> Result<Table> {
        let Some(head) = &self.head else {
            return Ok(Table::new());
        };
        let Some(bytes) = self
            .storage
            .read_files(head, &[DECLARATIONS_FILE])?
            .into_iter()
            .next()
            .flatten()
        else {
            return Ok(Table::new());
        };
        let context = || {
            format!(
                "cannot parse {DECLARATIONS_FILE} of the target `{}`",
                self.target
            )
        };
        String::from_utf8(bytes)
            .with_context(context)?
            .parse()
            .with_context(context)
    }
}

/// Shows the lists of the target, or of the primary target when `target` is `None`.
///
/// # Errors
///
/// Fails when this machine is not connected, the target does not exist or cannot be fetched, or its
/// `harness-sync.toml` does not parse.
pub fn show(paths: &Paths, target: Option<&str>) -> Result<Report> {
    let repo = Repo::open(paths, target)?;
    let listing = listing(&repo, &repo.table()?)?;
    Ok(Report {
        declarations: Some(listing),
        ..Report::new(
            "declarations show",
            format!("{DECLARATIONS_FILE} of the target `{}`", repo.target),
        )
    })
}

/// Adds `entries` to `list` of the target, as a commit that the target publishes.
///
/// # Errors
///
/// Fails as [`show`] does, and when the commit cannot be written or published.
pub fn add(paths: &Paths, target: Option<&str>, list: List, entries: &[KeyPath]) -> Result<Report> {
    change(paths, target, list, |values| {
        let before = values.len();
        for entry in entries {
            let value = Value::String(entry.to_string());
            if !values.contains(&value) {
                values.push(value);
            }
        }
        values.len() != before
    })
}

/// Removes `entries` from `list` of the target, as a commit that the target publishes.
///
/// # Errors
///
/// Fails as [`add`] does.
pub fn remove(
    paths: &Paths,
    target: Option<&str>,
    list: List,
    entries: &[KeyPath],
) -> Result<Report> {
    let entries: Vec<String> = entries.iter().map(ToString::to_string).collect();
    change(paths, target, list, |values| {
        let before = values.len();
        values.retain(|value| {
            !value
                .as_str()
                .is_some_and(|text| entries.iter().any(|entry| entry == text))
        });
        values.len() != before
    })
}

fn change(
    paths: &Paths,
    target: Option<&str>,
    list: List,
    edit: impl FnOnce(&mut Vec<Value>) -> bool,
) -> Result<Report> {
    let repo = Repo::open(paths, target)?;
    let mut table = repo.table()?;
    let key = list.key();
    let mut values = match table.remove(key) {
        None => Vec::new(),
        Some(Value::Array(values)) => values,
        Some(_) => bail!(
            "`{key}` in {DECLARATIONS_FILE} of the target `{}` is not a list",
            repo.target
        ),
    };
    let changed = edit(&mut values);
    if !values.is_empty() {
        table.insert(key.to_owned(), Value::Array(values));
    }
    let listing = listing(&repo, &table)?;
    if !changed {
        return Ok(Report {
            declarations: Some(listing),
            ..Report::new(
                "declarations",
                format!(
                    "nothing to change in `{key}` of the target `{}`",
                    repo.target
                ),
            )
        });
    }
    let content =
        toml::to_string(&table).with_context(|| format!("cannot serialize {DECLARATIONS_FILE}"))?;
    let parent = repo.head.as_deref();
    let mut tree = setup::excluded_removals(
        repo.storage.as_ref(),
        parent,
        &Declarations::parse(&content)?,
    )?;
    tree.push(FileChange::Write {
        path: DECLARATIONS_FILE.to_owned(),
        content: content.into_bytes(),
        executable: false,
    });
    let subject = format!("update `{key}` in {DECLARATIONS_FILE}");
    let commit = repo.storage.commit(parent, &tree, &subject)?;
    let (summary, pushed) = publish(&repo, key, commit)?;
    Ok(Report {
        declarations: Some(listing),
        pushes: vec![pushed],
        ..Report::new("declarations", summary)
    })
}

fn publish(repo: &Repo, key: &str, commit: String) -> Result<(String, TargetPush)> {
    let mut pushed = TargetPush {
        target: repo.target.clone(),
        outcome: Outcome::Ok,
        commit: Some(commit.clone()),
        review: None,
        items: Vec::new(),
    };
    let summary = match repo.storage.publish(&commit)? {
        Publish::SentForReview { change_url } => {
            let summary = format!(
                "sent the change of `{key}` for review as {}",
                change_url
                    .as_deref()
                    .unwrap_or_else(|| short_commit(&commit))
            );
            pushed.commit.clone_from(&repo.head);
            pushed.review = Some(Review { commit, change_url });
            summary
        }
        Publish::Published => {
            if repo.primary && repo.storage.base()? == repo.head {
                repo.storage.set_base(&commit)?;
            }
            format!(
                "updated `{key}` of the target `{}` as commit {}",
                repo.target,
                short_commit(&commit)
            )
        }
        Publish::RemoteAhead => bail!(
            "the target `{}` got a commit during the change: run the command again",
            repo.target
        ),
    };
    Ok((summary, pushed))
}

fn listing(repo: &Repo, table: &Table) -> Result<DeclarationsReport> {
    let list = |list: List| -> Result<Vec<String>> {
        let key = list.key();
        match table.get(key) {
            None => Ok(Vec::new()),
            Some(Value::Array(values)) => values
                .iter()
                .map(|value| {
                    value.as_str().map(str::to_owned).with_context(|| {
                        format!(
                            "`{key}` in {DECLARATIONS_FILE} holds an entry that is not a string"
                        )
                    })
                })
                .collect(),
            Some(_) => bail!("`{key}` in {DECLARATIONS_FILE} is not a list"),
        }
    };
    Ok(DeclarationsReport {
        target: repo.target.clone(),
        secrets: list(List::Secrets)?,
        include: list(List::Include)?,
        exclude: list(List::Exclude)?,
    })
}
