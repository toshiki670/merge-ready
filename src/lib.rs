//! merge-ready — Show pull request merge blockers as concise prompt tokens.

pub mod cli;
pub(crate) mod contexts;

use std::process::ExitCode;

use crate::contexts::daemon::application::cache as daemon_cache_app;
use crate::contexts::daemon::domain::cache::{RefreshMode, RepoId};
use crate::contexts::daemon::infrastructure::daemon_client::DaemonClient;
use crate::contexts::daemon::infrastructure::daemon_lifecycle::DaemonLifecycle;
use crate::contexts::daemon::interface::cli::daemon::DaemonCommand;
use crate::contexts::evaluation::infrastructure::toml_loader::TomlConfigRepository;
use crate::contexts::evaluation::infrastructure::{gh::GhClient, logger::Logger};
use crate::contexts::evaluation::interface::prompt::CacheHint;

fn cache_hint_to_refresh_mode(hint: CacheHint) -> RefreshMode {
    match hint {
        CacheHint::Hot => RefreshMode::Hot,
        CacheHint::Warm => RefreshMode::Warm,
        CacheHint::Terminal => RefreshMode::Terminal,
    }
}

fn build_daemon_lifecycle() -> DaemonLifecycle {
    DaemonLifecycle::new(
        // repo_id はブランチ変化を考慮して daemon_server が再導出して渡す
        |repo_id: &str, cwd: &std::path::Path| {
            let repo_id = RepoId::new(repo_id);
            let client = GhClient::new_in(cwd.to_path_buf(), Logger);
            let (output, hint) = contexts::evaluation::interface::prompt::render(
                &client,
                &TomlConfigRepository,
                &Logger,
            );
            let refresh_mode = cache_hint_to_refresh_mode(hint);
            daemon_cache_app::update(&DaemonClient, &repo_id, &output, refresh_mode);
        },
    )
}

/// Opens the configuration file in an editor.
///
/// Resolves the config path from `$XDG_CONFIG_HOME` or `$HOME`. If the file does not
/// exist it is created with default values before opening. Returns [`ExitCode::FAILURE`]
/// if the path cannot be determined or the editor invocation fails.
#[must_use]
pub fn config_command() -> ExitCode {
    let config_path = contexts::evaluation::infrastructure::toml_loader::config_path();
    contexts::evaluation::interface::cli::config::run(config_path.as_deref())
}

/// Starts the background cache daemon.
///
/// The daemon fetches PR merge-readiness in the background and caches the result
/// so that `merge-ready-prompt` can respond instantly.
#[must_use]
pub fn daemon_start_command() -> ExitCode {
    contexts::evaluation::infrastructure::logger::init();
    let lifecycle = build_daemon_lifecycle();
    contexts::daemon::interface::cli::daemon::run(DaemonCommand::Start, &lifecycle)
}

/// Stops the running background cache daemon.
#[must_use]
pub fn daemon_stop_command() -> ExitCode {
    contexts::evaluation::infrastructure::logger::init();
    let lifecycle = build_daemon_lifecycle();
    contexts::daemon::interface::cli::daemon::run(DaemonCommand::Stop, &lifecycle)
}

/// Shows the current status of the background cache daemon.
#[must_use]
pub fn daemon_status_command() -> ExitCode {
    contexts::evaluation::infrastructure::logger::init();
    let lifecycle = build_daemon_lifecycle();
    contexts::daemon::interface::cli::daemon::run(DaemonCommand::Status, &lifecycle)
}
