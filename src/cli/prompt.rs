pub(crate) fn run_check() {
    let tokens = crate::application::run(
        &crate::infra::gh::GhClient::new(),
        &crate::infra::logger::Logger,
        &crate::presentation::Presenter,
    );
    if !tokens.is_empty() {
        crate::presentation::display(&tokens);
    }
}
