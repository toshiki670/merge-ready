use clap::{Parser, Subcommand};

use super::prompt::PromptArgs;

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
    #[command(after_help = super::prompt::PROMPT_AFTER_HELP)]
    Prompt(PromptArgs),
}
