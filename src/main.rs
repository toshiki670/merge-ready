use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = merge_ready::cli::Cli::parse();
    merge_ready::cli::run(cli)
}
