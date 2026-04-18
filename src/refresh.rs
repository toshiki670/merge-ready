use merge_readiness_infrastructure::{cache, gh::GhClient, logger::Logger, refresh_lock};
use merge_readiness_interface::presentation::Presenter;

use crate::ConfigAdapter;

pub fn run(repo_id: &str) {
    let tokens = merge_readiness_application::prompt::fetch_output(&GhClient::new(), &Logger);
    if let Some(tokens) = tokens {
        let output = Presenter::new(ConfigAdapter::load()).render_to_string(&tokens);
        cache::write(repo_id, &output);
    }
    refresh_lock::release(repo_id);
}
