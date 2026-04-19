use clap::Subcommand;

use crate::contexts::status_cache::application::lifecycle::{self, Port};

#[derive(Subcommand, Clone, Copy)]
pub enum DaemonCommand {
    /// Start the background cache daemon (blocks; use as a background process)
    Start,
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
}

pub fn run(subcommand: DaemonCommand, port: &impl Port) {
    match subcommand {
        DaemonCommand::Start => start(port),
        DaemonCommand::Stop => stop(port),
        DaemonCommand::Status => status(port),
    }
}

fn start(port: &impl Port) {
    lifecycle::start(port);
}

fn stop(port: &impl Port) {
    if lifecycle::stop(port) {
        println!("daemon stopped");
    } else {
        eprintln!("daemon is not running");
    }
}

fn status(port: &impl Port) {
    match lifecycle::get_status(port) {
        Some(s) => {
            println!(
                "running  pid={}  entries={}  uptime={}s",
                s.pid, s.entries, s.uptime_secs
            );
        }
        None => println!("not running"),
    }
}
