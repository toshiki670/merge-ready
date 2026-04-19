use crate::contexts::merge_readiness::infrastructure::{
    cache, gh::GhClient, logger::Logger, refresh_lock,
};
use crate::contexts::merge_readiness::interface::presentation::Presenter;
use crate::contexts::status_cache::application::cache as status_cache;
use crate::contexts::status_cache::infrastructure::daemon_client::DaemonClient;

use crate::ConfigAdapter;

/// gh を直接呼んでキャッシュを更新する（stdout に出力しない）。
///
/// `repo_id` は親プロセスから `--repo-id` 引数で受け取る。
/// git から再取得しないため、ブランチ切替・git 一時失敗時でもロック解放が保証される。
pub fn run(repo_id: &str) {
    let tokens = crate::contexts::merge_readiness::application::prompt::fetch_output(
        &GhClient::new(),
        &Logger,
    );
    if let Some(tokens) = tokens {
        let output = Presenter::new(ConfigAdapter::load()).render_to_string(&tokens);
        // ファイルキャッシュに書き込む（デーモン未起動時のフォールバック用）
        cache::write(repo_id, &output);
        // デーモンが起動中であれば通知する（fire-and-forget）
        let client = DaemonClient;
        status_cache::update(&client, repo_id, &output);
    }
    refresh_lock::release(repo_id);
}
