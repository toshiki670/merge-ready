mod application;
mod domain;
mod infra;
mod presentation;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "merge-ready",
    about = "PR merge status for your shell prompt",
    version,
    after_help = "Shell integration:
  zsh:  PROMPT='$(merge-ready prompt) %# '
  bash: PS1='$(merge-ready prompt) \\$ '
  fish: function fish_prompt; echo -n (merge-ready prompt)' > '; end

Output tokens:
  ✓ merge-ready    Ready to merge
  ⚠ review         Review requested
  ⚠ ci-action      CI checks in progress
  ✗ ci-fail        CI checks failed
  ✗ conflict       Branch has merge conflicts
  ✗ update-branch  Branch is behind base branch
  ? sync-unknown   Branch sync status unknown
  ? loading        Status loading (first run)"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show PR merge status for your shell prompt
    Prompt,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Prompt) | None => run_check(),
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
