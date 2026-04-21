mod app;
mod cli;
mod contexts;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    app::run(cli)
}
