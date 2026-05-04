use std::process::ExitCode;

use clap::{Args, CommandFactory, Parser, Subcommand};

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
    /// Watch daemon cache entries in real time (Ctrl+C to stop)
    Watch,
}

#[derive(Args, Clone, Copy)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub subcommand: DaemonCommand,
}

#[derive(Subcommand, Clone, Copy)]
pub enum DaemonCommand {
    /// Start the background cache daemon
    Start,
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
}

#[must_use]
pub fn run(cli: &Cli) -> ExitCode {
    match &cli.command {
        Some(Command::Config) => merge_ready::config_command(),
        Some(Command::Daemon(args)) => match args.subcommand {
            DaemonCommand::Start => merge_ready::daemon_start_command(),
            DaemonCommand::Stop => merge_ready::daemon_stop_command(),
            DaemonCommand::Status => merge_ready::daemon_status_command(),
        },
        Some(Command::Watch) => merge_ready::watch_command(),
        None => {
            let _ = Cli::command().print_help();
            ExitCode::SUCCESS
        }
    }
}
