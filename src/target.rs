use anyhow::{Result, ensure};

use crate::config::{Config, Target};
use crate::git::{self, Git};
use crate::paths::Paths;
use crate::report::{Report, TargetReport, short_commit};

pub(crate) fn add(paths: &Paths, name: &str, target: Target) -> Result<Report> {
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
    let git = Git::open(paths.repo_dir());
    git.set_remote(name, &target.url)?;
    if let Err(err) = git.fetch(name) {
        git.remove_remote(name)?;
        return Err(err);
    }
    let head = git.resolve(&git::remote_ref(name, &target.branch))?;
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

pub(crate) fn remove(paths: &Paths, name: &str) -> Result<Report> {
    let config_file = paths.config_file();
    let mut config = Config::load_connected(&config_file)?;
    ensure!(
        config.primary != name,
        "`{name}` is the primary target; make another target primary first"
    );
    if config.targets.remove(name).is_none() {
        return Ok(Report::new(
            "target remove",
            format!("there is no target `{name}`"),
        ));
    }
    config.save(&config_file)?;
    Git::open(paths.repo_dir()).remove_remote(name)?;
    Ok(Report::new(
        "target remove",
        format!("removed the target `{name}`"),
    ))
}

pub(crate) fn set_primary(paths: &Paths, name: &str) -> Result<Report> {
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
    Git::open(paths.repo_dir()).clear_base()?;
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

pub(crate) fn list(paths: &Paths) -> Result<Report> {
    let config = Config::load_connected(&paths.config_file())?;
    let targets = config
        .targets
        .iter()
        .map(|(name, target)| TargetReport {
            name: name.clone(),
            url: target.url.clone(),
            branch: target.branch.clone(),
            kind: target.kind,
            primary: *name == config.primary,
        })
        .collect();
    Ok(Report {
        targets,
        ..Report::new("target list", format!("primary target: {}", config.primary))
    })
}
