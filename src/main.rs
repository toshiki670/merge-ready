mod application;
mod domain;
mod infra;
mod presentation;

fn main() {
    let tokens = application::run(
        &infra::gh::GhClient::new(),
        &infra::logger::Logger,
        &presentation::Presenter,
    );
    if !tokens.is_empty() {
        presentation::display(&tokens);
    }
}
