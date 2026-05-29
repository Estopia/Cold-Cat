mod engine;

#[cfg(windows)]
mod win_service;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "coldcat-daemon")]
#[command(about = "Cold Cat privileged blocking daemon")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run in the foreground for development or systemd.
    Run,
    /// Run under the Windows Service Control Manager.
    #[cfg(windows)]
    Service,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command.unwrap_or(Command::Run) {
        Command::Run => engine::run_console(),
        #[cfg(windows)]
        Command::Service => win_service::run(),
    }
}
