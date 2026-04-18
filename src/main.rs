mod cached;
mod refresh;

use clap::{CommandFactory, Parser, Subcommand};
use merge_readiness_application::prompt::{ExecutionMode, RepoIdPort};
use merge_readiness_infrastructure::{gh::GhClient, logger::Logger};
use merge_readiness_interface::cli::prompt::{self, PromptArgs, AFTER_HELP};

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
    #[command(after_help = AFTER_HELP)]
    Prompt(PromptArgs),
}

struct InfraRepoIdPort;

impl RepoIdPort for InfraRepoIdPort {
    fn get(&self) -> Option<String> {
        merge_readiness_infrastructure::repo_id::get()
    }
}

fn main() {
    let repo_id_port = InfraRepoIdPort;
    let Some(mode) = (match Cli::parse().command {
        Some(Command::Prompt(args)) => prompt::resolve_mode(&args, &repo_id_port),
        None => {
            let _ = Cli::command().print_help();
            None
        }
    }) else {
        return;
    };

    match mode {
        ExecutionMode::Direct => {
            merge_readiness_interface::cli::prompt::direct::run(&GhClient::new(), &Logger);
        }
        ExecutionMode::Cached => cached::run(),
        ExecutionMode::BackgroundRefresh { repo_id } => refresh::run(&repo_id),
    }
}
