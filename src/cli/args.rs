use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "merge-ready",
    about = "PR merge status for your shell prompt",
    version
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Show PR merge status for your shell prompt
    #[command(after_help = super::help::PROMPT_AFTER_HELP)]
    Prompt(PromptArgs),
}

#[derive(Args)]
pub(crate) struct PromptArgs {
    /// Bypass cache and fetch fresh data directly
    #[arg(long)]
    pub(crate) no_cache: bool,
    /// Fetch fresh data and update cache without displaying output
    #[arg(long, hide = true, conflicts_with = "no_cache")]
    pub(crate) refresh: bool,
    /// Repository ID for lock release (passed by parent process via --refresh)
    #[arg(long, hide = true, requires = "refresh")]
    pub(crate) repo_id: Option<String>,
}
