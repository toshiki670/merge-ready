mod application;
mod domain;
mod infra;
mod presentation;

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "merge-ready",
    about = "PR merge status for your shell prompt",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show PR merge status for your shell prompt
    #[command(after_help = "Output tokens:
  ✓ merge-ready    Ready to merge
  ⚠ review         Review requested
  ⚠ ci-action      CI checks in progress
  ✗ ci-fail        CI checks failed
  ✗ conflict       Branch has merge conflicts
  ✗ update-branch  Branch is behind base branch
  ? sync-unknown   Branch sync status unknown")]
    Prompt,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Prompt) => run_check(),
        None => {
            let _ = Cli::command().print_help();
        }
    }
}

fn run_check() {
    let tokens = application::run(
        &infra::gh::GhClient::new(),
        &infra::logger::Logger,
        &presentation::Presenter,
    );
    if !tokens.is_empty() {
        presentation::display(&tokens);
    }
}
