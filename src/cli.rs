mod args;
mod prompt;

use args::{Cli, Command};
use clap::{CommandFactory, Parser};

pub(crate) fn run() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Prompt(args)) => prompt::run(&args),
        None => {
            let _ = Cli::command().print_help();
        }
    }
}
