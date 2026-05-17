//! merge-ready — Show pull request merge blockers as concise prompt tokens.

pub(crate) mod contexts;
pub(crate) mod shared;

use std::process::ExitCode;

use crate::contexts::daemon::domain::cache::{CachePort, RepoId};
use crate::contexts::daemon::infrastructure::daemon_client::DaemonClient;
use crate::contexts::daemon::infrastructure::daemon_lifecycle::DaemonLifecycle;
use crate::contexts::daemon::infrastructure::paths::Paths;
use crate::contexts::evaluation::infrastructure::toml_loader::TomlConfigRepository;
use crate::contexts::evaluation::infrastructure::{gh::GhClient, logger::Logger};
use crate::shared::protocol::PrOutput;

fn build_daemon_lifecycle() -> DaemonLifecycle {
    DaemonLifecycle::new(
        // repo_id はブランチ変化を考慮して daemon_server が再導出して渡す
        |repo_id: &RepoId, cwd: &std::path::Path| {
            let client = GhClient::new_in(cwd.to_path_buf(), Logger);
            let result = contexts::evaluation::interface::prompt::render(
                &client,
                &TomlConfigRepository::new(),
                &Logger,
            );
            let pr_outputs = result
                .pr_outputs
                .into_iter()
                .map(|(pr_id, output)| PrOutput {
                    pr_id: pr_id.as_u64(),
                    output,
                })
                .collect();
            DaemonClient::new(Paths::default().socket_path()).update(
                repo_id,
                &result.output,
                result.refresh_mode,
                pr_outputs,
            );
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
    contexts::daemon::interface::cli::daemon::start(&lifecycle)
}

/// Stops the running background cache daemon.
#[must_use]
pub fn daemon_stop_command() -> ExitCode {
    contexts::evaluation::infrastructure::logger::init();
    let lifecycle = build_daemon_lifecycle();
    contexts::daemon::interface::cli::daemon::stop(&lifecycle)
}

/// Shows the current status of the background cache daemon.
#[must_use]
pub fn daemon_status_command() -> ExitCode {
    contexts::evaluation::infrastructure::logger::init();
    let lifecycle = build_daemon_lifecycle();
    contexts::daemon::interface::cli::daemon::status(&lifecycle)
}

/// Watches daemon cache entries in real time.
#[must_use]
pub fn watch_command() -> ExitCode {
    let lifecycle = build_daemon_lifecycle();
    contexts::daemon::interface::cli::watch::run(&lifecycle)
}
