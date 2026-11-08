//! harness-sync syncs the user-level setup of coding agent harnesses between machines through a git repo.

mod output;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use harness_sync_core::declare::List;
use harness_sync_core::item::{ItemId, KeyPath};
use harness_sync_core::paths::Paths;
use harness_sync_core::pull::{Choices, Prompts, Side, Take};
use harness_sync_core::registry::Registry;
use harness_sync_core::report::Report;
use harness_sync_core::{
    config, declare, harnesses, init, pin, pull, push, registry, set_secret, status, target,
};
use harness_sync_git::{GERRIT, GIT};

use crate::output::Terminal;

static REGISTRY: Registry = Registry {
    harnesses: &[
        &harness_sync_claude_code::HARNESS,
        &harness_sync_pi::HARNESS,
    ],
    storage_kinds: &[&GIT, &GERRIT],
};

/// Sync the user-level setup of coding agent harnesses between machines through a git repo.
#[derive(Debug, Parser)]
#[command(
    version,
    about,
    after_help = "Exit codes: 0 success, 1 failure, 2 usage error, 3 pull stopped on conflicts, \
                  4 push refused because the remote is ahead of the base commit, \
                  5 push blocked by the secret scan"
)]
struct Cli {
    /// Print the report as JSON
    #[arg(long, global = true)]
    json: bool,
    /// Never prompt; settle each choice with flags
    #[arg(long, global = true)]
    no_input: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Connect this machine to the git repo that holds the remote snapshot
    Init {
        /// URL or path of the git remote
        remote: String,
        /// Branch that holds the remote snapshot
        #[arg(long, default_value = "main")]
        branch: String,
        /// Name of the primary target that the remote becomes
        #[arg(long, default_value = "origin", value_parser = config::parse_target_name)]
        name: String,
        /// The remote is a Gerrit project: push sends each snapshot as a change for review
        #[arg(long)]
        gerrit: bool,
        /// Harness that this machine syncs; repeat it for several. Without it, each harness whose directory exists
        #[arg(long = "harness", value_name = "NAME")]
        harnesses: Vec<String>,
    },
    /// List the supported items that differ from the remote snapshot
    Status,
    /// Send the local setup to the primary target, or to the targets that --to names
    Push {
        /// Target to send to instead of the primary, or `all`; repeat it for several targets
        #[arg(long = "to", value_name = "TARGET")]
        to: Vec<String>,
    },
    /// Apply the remote snapshot to the local setup
    Pull {
        /// Side that wins every conflict without a --take
        #[arg(long, value_enum)]
        prefer: Option<SideArg>,
        /// Side that wins the conflict of one item, as ITEM=local or ITEM=remote
        #[arg(long = "take", value_name = "ITEM=SIDE", value_parser = parse_take)]
        takes: Vec<Take>,
    },
    /// Keep an item as it is on this machine: push and pull leave it alone
    Pin {
        /// Item id, for example `.claude/settings.json:/statusLine`
        item: ItemId,
    },
    /// Let push and pull move a pinned item again
    Unpin {
        /// Item id, for example `.claude/settings.json:/statusLine`
        item: ItemId,
    },
    /// Manage the secrets of this machine
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
    /// Manage the git repos that this machine pushes to
    Target {
        #[command(subcommand)]
        command: TargetCommand,
    },
    /// Manage the harnesses that this machine syncs
    Harness {
        #[command(subcommand)]
        command: HarnessCommand,
    },
    /// Show or change the secrets, include, and exclude lists in harness-sync.toml of a target
    Declarations {
        #[command(subcommand)]
        command: DeclarationsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DeclarationsCommand {
    /// Show the lists of a target
    Show {
        /// Target to read instead of the primary
        #[arg(long)]
        target: Option<String>,
    },
    /// Add entries to a list, as a commit to the target
    Add {
        /// List to change
        #[arg(value_enum)]
        list: ListArg,
        /// Item ids or directory paths, for example `.claude/settings.json:/env/NAME` or `.claude/hooks`
        #[arg(required = true)]
        entries: Vec<KeyPath>,
        /// Target to change instead of the primary
        #[arg(long)]
        target: Option<String>,
    },
    /// Remove entries from a list, as a commit to the target
    Remove {
        /// List to change
        #[arg(value_enum)]
        list: ListArg,
        /// Item ids or directory paths to remove
        #[arg(required = true)]
        entries: Vec<KeyPath>,
        /// Target to change instead of the primary
        #[arg(long)]
        target: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum TargetCommand {
    /// Add a target that push can send the snapshot to
    Add {
        /// Name of the target
        #[arg(value_parser = config::parse_target_name)]
        name: String,
        /// URL or path of the git remote
        url: String,
        /// Branch that holds the snapshot
        #[arg(long, default_value = "main")]
        branch: String,
        /// The target is a Gerrit project: push sends each snapshot as a change for review
        #[arg(long)]
        gerrit: bool,
    },
    /// Remove a target that is not the primary
    Remove {
        /// Name of the target
        name: String,
    },
    /// Make a target the primary: status and pull read it, and push sends to it
    Primary {
        /// Name of the target
        name: String,
    },
    /// List the targets of this machine
    List,
}

#[derive(Debug, Subcommand)]
enum HarnessCommand {
    /// Sync a harness on this machine
    Add {
        /// Name of the harness, for example `claude-code`
        name: String,
    },
    /// Stop syncing a harness on this machine; its files stay as they are
    Remove {
        /// Name of the harness
        name: String,
    },
    /// List the harnesses that harness-sync knows and the ones this machine syncs
    List,
}

#[derive(Debug, Subcommand)]
enum SecretCommand {
    /// Set a secret value on this machine, from a TTY prompt or from stdin
    Set {
        /// Item id of the secret, for example `.claude/settings.json:/env/GITHUB_TOKEN`
        item_id: KeyPath,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let printed =
        execute(&cli).and_then(|report| output::report(&report, cli.json).map(|()| report.outcome));
    match printed {
        Ok(outcome) => ExitCode::from(outcome.exit_code()),
        Err(err) => {
            output::error(&err);
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: &Cli) -> Result<Report> {
    registry::install(&REGISTRY)?;
    let paths = Paths::from_env()?;
    let prompts = output::can_prompt(cli.no_input).then_some(&Terminal as &dyn Prompts);
    match &cli.command {
        Command::Init {
            remote,
            branch,
            name,
            gerrit,
            harnesses,
        } => init::run(&paths, name, target(remote, branch, *gerrit), harnesses),
        Command::Status => status::run(&paths),
        Command::Push { to } => push::run(&paths, to),
        Command::Pull { prefer, takes } => pull::run(
            &paths,
            &Choices {
                prefer: prefer.map(Side::from),
                takes,
            },
            prompts,
        ),
        Command::Pin { item } => pin::run(&paths, item, true),
        Command::Unpin { item } => pin::run(&paths, item, false),
        Command::Secret {
            command: SecretCommand::Set { item_id },
        } => set_secret::run(&paths, item_id, || output::read_secret(item_id, prompts)),
        Command::Target { command } => match command {
            TargetCommand::Add {
                name,
                url,
                branch,
                gerrit,
            } => target::add(&paths, name, target(url, branch, *gerrit)),
            TargetCommand::Remove { name } => target::remove(&paths, name),
            TargetCommand::Primary { name } => target::set_primary(&paths, name),
            TargetCommand::List => target::list(&paths),
        },
        Command::Harness { command } => match command {
            HarnessCommand::Add { name } => harnesses::add(&paths, name),
            HarnessCommand::Remove { name } => harnesses::remove(&paths, name),
            HarnessCommand::List => harnesses::list(&paths),
        },
        Command::Declarations { command } => match command {
            DeclarationsCommand::Show { target } => declare::show(&paths, target.as_deref()),
            DeclarationsCommand::Add {
                list,
                entries,
                target,
            } => declare::add(&paths, target.as_deref(), (*list).into(), entries),
            DeclarationsCommand::Remove {
                list,
                entries,
                target,
            } => declare::remove(&paths, target.as_deref(), (*list).into(), entries),
        },
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SideArg {
    Local,
    Remote,
}

impl From<SideArg> for Side {
    fn from(side: SideArg) -> Self {
        match side {
            SideArg::Local => Self::Local,
            SideArg::Remote => Self::Remote,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ListArg {
    Secrets,
    Include,
    Exclude,
}

impl From<ListArg> for List {
    fn from(list: ListArg) -> Self {
        match list {
            ListArg::Secrets => Self::Secrets,
            ListArg::Include => Self::Include,
            ListArg::Exclude => Self::Exclude,
        }
    }
}

fn parse_take(text: &str) -> Result<Take, String> {
    let (id, side) = text
        .rsplit_once('=')
        .ok_or_else(|| format!("`{text}` is not ITEM=local or ITEM=remote"))?;
    Ok(Take {
        id: id.parse().map_err(|err: anyhow::Error| err.to_string())?,
        side: SideArg::from_str(side, true)?.into(),
    })
}

fn target(url: &str, branch: &str, gerrit: bool) -> config::Target {
    let kind = if gerrit { &GERRIT } else { &GIT };
    config::Target {
        url: url.to_owned(),
        branch: branch.to_owned(),
        kind: kind.name.to_owned(),
    }
}
