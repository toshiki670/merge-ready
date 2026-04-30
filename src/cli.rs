use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};

use crate::contexts::daemon::interface::cli::DaemonArgs;

#[derive(Parser)]
#[command(
    name = "merge-ready",
    about = "PR merge status for your shell prompt",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Open the configuration file in an editor (creates it with defaults if absent)
    Config,
    /// Manage the background cache daemon
    Daemon(DaemonArgs),
}

#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Some(Command::Config) => crate::config_command(),
        Some(Command::Daemon(args)) => crate::daemon_command(args),
        None => {
            let _ = Cli::command().print_help();
            ExitCode::SUCCESS
        }
    }
}
