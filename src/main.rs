mod adapters;
mod app;
mod cli;
mod contexts;
#[allow(dead_code)]
mod protocol {
    include!("protocol/protocol.rs");
}

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    app::run(cli)
}
