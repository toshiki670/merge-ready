use std::process::ExitCode;

use clap::{Args, Subcommand};

use crate::contexts::configuration::infrastructure::toml_loader::{
    TomlConfigRepository, config_path,
};

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

pub fn run(args: &ConfigArgs) -> ExitCode {
    match args.subcommand {
        ConfigCommand::Edit => {
            let Some(path) = config_path() else {
                eprintln!(
                    "failed to edit config: could not determine config path (HOME or XDG_CONFIG_HOME required)"
                );
                return ExitCode::FAILURE;
            };
            if let Err(e) = edit::run(&path) {
                eprintln!("failed to edit config: {e}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        ConfigCommand::Update => {
            if let Err(e) = update::run(&TomlConfigRepository) {
                eprintln!("failed to update config: {e}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
    }
}
