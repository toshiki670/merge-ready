mod args;
mod help;
mod prompt;

use args::{Cli, Command};
use clap::{CommandFactory, Parser};

pub(crate) fn run() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Prompt) => prompt::run_check(),
        None => {
            let _ = Cli::command().print_help();
        }
    }
}
