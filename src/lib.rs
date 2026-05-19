//! merge-ready — Show pull request merge blockers as concise prompt tokens.

pub(crate) mod contexts;
pub(crate) mod shared;

pub use shared::prompt_ipc;

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::ExitCode;

use crate::contexts::daemon::domain::cache::{CachePort, RepoId};
use crate::contexts::daemon::infrastructure::daemon_client::DaemonClient;
use crate::contexts::daemon::infrastructure::daemon_lifecycle::DaemonLifecycle;
use crate::contexts::daemon::infrastructure::paths::Paths;
use crate::contexts::evaluation::application::errors::into_token;
use crate::contexts::evaluation::infrastructure::gh::fetch_prompt;
use crate::contexts::evaluation::infrastructure::logger::log_repository_error;
use crate::contexts::evaluation::infrastructure::toml_loader::load_display_config;
use crate::contexts::evaluation::interface::prompt::render;
use crate::shared::protocol::PrOutput;

/// Imperative Shell: 副作用（gh サブプロセス、TOML 読み込み、ログ書き込み、
/// daemon キャッシュ更新）を集約して純関数 `render` を駆動する。
fn refresh_callback(
    repo_id: RepoId,
    cwd: PathBuf,
) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
    Box::pin(async move {
        let prompt_result = fetch_prompt(&cwd).await.map_err(|e| {
            log_repository_error(e);
            into_token(e)
        });
        let config = load_display_config();
        let result = render(prompt_result, &config);

        let pr_outputs = result
            .pr_outputs
            .into_iter()
            .map(|(pr_id, output)| PrOutput {
                pr_id: pr_id.as_u64(),
                output,
            })
            .collect();
        DaemonClient::new(Paths::default().socket_path()).update(
            &repo_id,
            &result.output,
            result.refresh_mode,
            pr_outputs,
        );
    })
}

fn build_daemon_lifecycle() -> DaemonLifecycle {
    DaemonLifecycle::new(refresh_callback)
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
pub async fn daemon_start_command() -> ExitCode {
    contexts::evaluation::infrastructure::logger::init();
    let lifecycle = build_daemon_lifecycle();
    contexts::daemon::interface::cli::daemon::start(&lifecycle).await
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
