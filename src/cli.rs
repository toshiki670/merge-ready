use std::io;
use std::process::ExitCode;

use clap::CommandFactory;
use clap_complete::generate;

pub use crate::cli_args::{Cli, Command, DaemonCommand};

#[must_use]
pub async fn run(cli: &Cli) -> ExitCode {
    match &cli.command {
        Some(Command::Config) => merge_ready::config_command(),
        Some(Command::Completions { shell }) => {
            let mut cmd = Cli::command();
            generate(*shell, &mut cmd, "merge-ready", &mut io::stdout());
            ExitCode::SUCCESS
        }
        Some(Command::Daemon(args)) => match args.subcommand {
            DaemonCommand::Start => merge_ready::daemon_start_command().await,
            DaemonCommand::Stop => merge_ready::daemon_stop_command(),
            DaemonCommand::Status => merge_ready::daemon_status_command(),
        },
        Some(Command::Watch) => merge_ready::watch_command().await,
        None => {
            let _ = Cli::command().print_help();
            ExitCode::SUCCESS
        }
    }
}
