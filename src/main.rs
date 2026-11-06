//! cc-sync syncs the Claude Code user-level setup between machines through a git repo.

mod atomic;
mod compare;
mod config;
mod git;
mod init;
mod item;
mod output;
mod paths;
mod push;
mod report;
mod session;
mod setup;
mod status;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::paths::Paths;
use crate::report::Report;

/// Sync the Claude Code user-level setup between machines through a git repo.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Print the report as JSON
    #[arg(long, global = true)]
    json: bool,
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
    match &cli.command {
        Command::Init { remote, branch } => init::run(&paths, remote, branch),
        Command::Status => status::run(&paths),
        Command::Push => push::run(&paths),
    }
}
