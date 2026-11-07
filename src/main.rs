//! harness-sync syncs the Claude Code user-level setup between machines through a git repo.

mod atomic;
mod compare;
mod config;
mod declarations;
mod declare;
mod executables;
mod files;
mod gerrit;
mod git;
mod harness;
mod harnesses;
mod home;
mod init;
mod item;
mod json_edit;
mod output;
mod paths;
mod pin;
mod pull;
mod push;
mod references;
mod report;
mod secret;
mod session;
mod set_secret;
mod setup;
mod status;
mod target;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::item::{ItemId, KeyPath};
use crate::paths::Paths;
use crate::pull::{Choices, Side, Take};
use crate::report::Report;

/// Sync the Claude Code user-level setup between machines through a git repo.
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
        prefer: Option<Side>,
        /// Side that wins the conflict of one item, as ITEM=local or ITEM=remote
        #[arg(long = "take", value_name = "ITEM=SIDE", value_parser = pull::parse_take)]
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
        list: declare::List,
        /// Key paths or item ids, for example `.claude/settings.json:/extraKnownMarketplaces/NAME`
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
        list: declare::List,
        /// Key paths or item ids to remove
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
        /// Key path of the secret, for example `.claude/settings.json:/env/GITHUB_TOKEN`
        key_path: KeyPath,
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
    let paths = Paths::from_env()?;
    let prompts = output::can_prompt(cli.no_input);
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
                prefer: *prefer,
                takes,
            },
            prompts,
        ),
        Command::Pin { item } => pin::run(&paths, item, true),
        Command::Unpin { item } => pin::run(&paths, item, false),
        Command::Secret {
            command: SecretCommand::Set { key_path },
        } => set_secret::run(&paths, key_path, prompts),
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
            } => declare::add(&paths, target.as_deref(), *list, entries),
            DeclarationsCommand::Remove {
                list,
                entries,
                target,
            } => declare::remove(&paths, target.as_deref(), *list, entries),
        },
    }
}

fn target(url: &str, branch: &str, gerrit: bool) -> config::Target {
    config::Target {
        url: url.to_owned(),
        branch: branch.to_owned(),
        kind: config::Kind::from_gerrit_flag(gerrit),
    }
}
