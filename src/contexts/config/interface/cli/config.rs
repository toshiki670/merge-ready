use std::path::Path;
use std::process::ExitCode;

use clap::{Args, Subcommand};

use crate::contexts::config::application::port::UpdateConfigPort;

pub mod edit;
pub mod update;

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub subcommand: ConfigCommand,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Open the configuration file in an editor (creates it with defaults if absent)
    Edit,
    /// Update the configuration file to the latest schema (preserves valid keys, removes obsolete ones, adds missing ones with defaults)
    Update,
}

pub fn run(
    args: &ConfigArgs,
    port: &impl UpdateConfigPort,
    config_path: Option<&Path>,
) -> ExitCode {
    match args.subcommand {
        ConfigCommand::Edit => {
            let Some(path) = config_path else {
                eprintln!(
                    "failed to edit config: could not determine config path (HOME or XDG_CONFIG_HOME required)"
                );
                return ExitCode::FAILURE;
            };
            if let Err(e) = edit::run(path) {
                eprintln!("failed to edit config: {e}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        ConfigCommand::Update => {
            if let Err(e) = update::run(port) {
                eprintln!("failed to update config: {e}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
    }
}
