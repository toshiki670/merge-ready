use clap::{Parser, Subcommand};

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
