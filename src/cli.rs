use clap::{Parser, Subcommand};

use crate::contexts::configuration::interface::cli::ConfigArgs;
use crate::contexts::merge_readiness::interface::cli::prompt::{AFTER_HELP, PromptArgs};
use crate::contexts::status_cache::interface::cli::DaemonArgs;

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
    /// Show PR merge status for your shell prompt
    #[command(after_help = AFTER_HELP)]
    Prompt(PromptArgs),
    /// Manage the configuration file
    Config(ConfigArgs),
    /// Manage the background cache daemon
    Daemon(DaemonArgs),
}
