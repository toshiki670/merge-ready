mod application;
mod domain;
mod infra;
mod presentation;

fn main() {
    application::run(&infra::gh::GhClient::new());
}
