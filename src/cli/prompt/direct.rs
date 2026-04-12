/// gh を直接呼んで結果を stdout に出力する（キャッシュを使わない）。
pub(super) fn run_direct() {
    let tokens = crate::application::run(
        &crate::infra::gh::GhClient::new(),
        &crate::infra::logger::Logger,
        &crate::presentation::Presenter,
    );
    if !tokens.is_empty() {
        crate::presentation::display(&tokens);
    }
}
