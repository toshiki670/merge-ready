use std::path::Path;
use std::process::ExitCode;

use clap::{Args, Subcommand};

pub mod edit;

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub subcommand: ConfigCommand,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Open the configuration file in an editor (creates it with defaults if absent)
    Edit,
}

pub fn run(args: &ConfigArgs, config_path: Option<&Path>) -> ExitCode {
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
    }
}
