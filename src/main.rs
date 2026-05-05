mod cli;
mod cli_args;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    cli::run(&cli)
}
