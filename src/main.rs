//! cc-sync syncs the Claude Code user-level setup between machines through a git repo.

mod atomic;
mod compare;
mod config;
mod declarations;
mod git;
mod home;
mod init;
mod item;
mod json_edit;
mod output;
mod paths;
mod pin;
mod pull;
mod push;
mod report;
mod secret;
mod session;
mod set_secret;
mod setup;
mod status;

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
    },
    /// List the supported items that differ from the remote snapshot
    Status,
    /// Send the local setup to the remote snapshot
    Push,
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
        /// Item id, for example `settings:/statusLine`
        item: ItemId,
    },
    /// Let push and pull move a pinned item again
    Unpin {
        /// Item id, for example `settings:/statusLine`
        item: ItemId,
    },
    /// Manage the secrets of this machine
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SecretCommand {
    /// Set a secret value on this machine, from a TTY prompt or from stdin
    Set {
        /// Key path of the secret, for example `settings:/env/GITHUB_TOKEN`
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
        Command::Init { remote, branch } => init::run(&paths, remote, branch),
        Command::Status => status::run(&paths),
        Command::Push => push::run(&paths),
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
    }
}
