mod application;
mod cli;
mod domain;
mod infra;
mod presentation;

use application::cache::DisplayAction;

fn main() {
    cli::run();
}
