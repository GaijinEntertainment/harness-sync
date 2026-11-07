use std::slice;

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use toml::{Table, Value};

use crate::config::{Config, Kind};
use crate::declarations::DECLARATIONS_FILE;
use crate::gerrit;
use crate::git::{self, BASE_REF, Git, TreeChange};
use crate::item::KeyPath;
use crate::paths::Paths;
use crate::report::{DeclarationsReport, Report, Review, TargetPush, short_commit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum List {
    Secrets,
    Include,
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
    git: Git,
    target: String,
    branch: String,
    kind: Kind,
    primary: bool,
    head: Option<String>,
}

impl Repo {
    fn open(paths: &Paths, target: Option<&str>) -> Result<Self> {
        let config = Config::load_connected(&paths.config_file())?;
        let target = target.unwrap_or(&config.primary).to_owned();
        let (branch, kind) = config
            .targets
            .get(&target)
            .map(|found| (found.branch.clone(), found.kind))
            .with_context(|| {
                format!("there is no target `{target}`; add it with `cc-sync target add`")
            })?;
        let git = Git::open(paths.repo_dir());
        git.fetch(&target)?;
        let head = git.resolve(&git::remote_ref(&target, &branch))?;
        Ok(Self {
            primary: target == config.primary,
            git,
            target,
            branch,
            kind,
            head,
        })
    }

    fn table(&self) -> Result<Table> {
        let Some(head) = &self.head else {
            return Ok(Table::new());
        };
        let spec = format!("{head}:{DECLARATIONS_FILE}");
        let Some(bytes) = self
            .git
            .read_files(slice::from_ref(&spec))?
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

pub(crate) fn show(paths: &Paths, target: Option<&str>) -> Result<Report> {
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

pub(crate) fn add(
    paths: &Paths,
    target: Option<&str>,
    list: List,
    entries: &[KeyPath],
) -> Result<Report> {
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

pub(crate) fn remove(
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
    let content = toml::to_string(&table)
        .with_context(|| format!("cannot serialize {DECLARATIONS_FILE}"))?
        .into_bytes();
    let parent = repo.head.as_deref();
    let subject = format!("update `{key}` in {DECLARATIONS_FILE}");
    let message = gerrit::message(&repo.git, repo.kind, subject, parent)?;
    let file = TreeChange::Write {
        path: DECLARATIONS_FILE.to_owned(),
        content,
        executable: false,
    };
    let commit = repo.git.commit(parent, slice::from_ref(&file), &message)?;
    let remote_messages = repo.git.push(
        &repo.target,
        &commit,
        &gerrit::destination(repo.kind, &repo.branch),
    )?;
    let mut report = Report {
        declarations: Some(listing),
        ..Report::new("declarations", String::new())
    };
    let mut pushed = TargetPush {
        target: repo.target.clone(),
        outcome: report.outcome,
        commit: Some(commit.clone()),
        review: None,
        items: Vec::new(),
    };
    if repo.kind == Kind::Gerrit {
        let change_url = gerrit::change_url(&remote_messages);
        report.summary = format!(
            "sent the change of `{key}` for review as {}",
            change_url
                .as_deref()
                .unwrap_or_else(|| short_commit(&commit))
        );
        pushed.commit.clone_from(&repo.head);
        pushed.review = Some(Review { commit, change_url });
    } else {
        if repo.primary && repo.git.resolve(BASE_REF)? == repo.head {
            repo.git.set_base(&commit)?;
        }
        report.summary = format!(
            "updated `{key}` of the target `{}` as commit {}",
            repo.target,
            short_commit(&commit)
        );
    }
    report.pushes.push(pushed);
    Ok(report)
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
