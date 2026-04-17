mod args;
pub mod prompt;

use args::{Cli, Command};
use clap::{CommandFactory, Parser};
use merge_readiness_application::prompt::{ExecutionMode, RepoIdPort};

pub fn run(repo_id_port: &impl RepoIdPort) -> Option<ExecutionMode> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Prompt(args)) => prompt::resolve_mode(&args, repo_id_port),
        None => {
            let _ = Cli::command().print_help();
            None
        }
    }
}
