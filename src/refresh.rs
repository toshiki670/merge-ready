use crate::contexts::merge_readiness::infrastructure::{gh::GhClient, logger::Logger};
use crate::contexts::merge_readiness::interface::presentation::Presenter;
use crate::contexts::status_cache::application::cache as status_cache;
use crate::contexts::status_cache::infrastructure::daemon_client::DaemonClient;

use crate::ConfigAdapter;

/// gh を直接呼んで daemon キャッシュを更新する（stdout に出力しない）。
///
/// `repo_id` は `daemon refresh --repo-id` 引数で受け取る。
pub fn run(repo_id: &str) {
    let tokens = crate::contexts::merge_readiness::application::prompt::fetch_output(
        &GhClient::new(),
        &Logger,
    );
    if let Some(tokens) = tokens {
        let output = Presenter::new(ConfigAdapter::load()).render_to_string(&tokens);
        status_cache::update(&DaemonClient, repo_id, &output);
    }
}
