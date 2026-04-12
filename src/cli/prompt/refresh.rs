/// gh を直接呼んでキャッシュを更新する（stdout に出力しない）。
///
/// `repo_id` は親プロセスから `--repo-id` 引数で受け取る。
/// git から再取得しないため、ブランチ切替・git 一時失敗時でもロック解放が保証される。
pub(super) fn run_refresh(repo_id: &str) {
    let tokens = crate::application::prompt::fetch_output(
        &crate::infra::gh::GhClient::new(),
        &crate::infra::logger::Logger,
    );
    if let Some(tokens) = tokens {
        let output = crate::presentation::render_to_string(&tokens);
        crate::infra::cache::write(repo_id, &output);
    }
    crate::infra::refresh_lock::release(repo_id);
}
