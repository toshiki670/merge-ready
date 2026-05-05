use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

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
    /// Generate shell completion scripts
    Completions {
        /// The shell to generate completions for
        shell: Shell,
    },
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
