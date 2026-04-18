use merge_readiness_infrastructure::{cache, gh::GhClient, logger::Logger, refresh_lock};
use merge_readiness_interface::presentation::Presenter;

use crate::ConfigAdapter;

/// gh を直接呼んでキャッシュを更新する（stdout に出力しない）。
///
/// `repo_id` は親プロセスから `--repo-id` 引数で受け取る。
/// git から再取得しないため、ブランチ切替・git 一時失敗時でもロック解放が保証される。
pub fn run(repo_id: &str) {
    let tokens = merge_readiness_application::prompt::fetch_output(&GhClient::new(), &Logger);
    if let Some(tokens) = tokens {
        let output = Presenter::new(ConfigAdapter::load()).render_to_string(&tokens);
        cache::write(repo_id, &output);
    }
    refresh_lock::release(repo_id);
}
