//! cc-sync syncs the Claude Code user-level setup between machines through a git repo.

use clap::Parser;

/// Sync the Claude Code user-level setup between machines through a git repo.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {}

fn main() {
    Cli::parse();
}
