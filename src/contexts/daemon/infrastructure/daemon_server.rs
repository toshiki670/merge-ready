use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::connection;
use super::daemon_state_actor;
use super::paths::Paths;
use super::rate_limit_client::RateLimitClient;
use super::rate_limit_fetcher;
use super::repo_id;
use super::restart;
use super::scheduler;
use super::server_config;
use super::signals;
use super::socket_listener;
use super::socket_watcher;
use crate::contexts::daemon::domain::cache::RepoId;
use crate::contexts::daemon::domain::daemon::DaemonError;

/// Imperative Shell から渡される、副作用を含むリフレッシュ実装。
/// async 関数ポインタとして受け取り、キャプチャを禁じ依存を明示する。
pub(super) type RefreshFn =
    fn(RepoId, PathBuf) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// デーモンのメインループ。ソケットをバインドして接続を待ち受ける。
///
/// `on_refresh` はキャッシュ更新が必要になったときに tokio タスクとして spawn される。
/// Stop リクエストで `Ok(())` を返す。
pub async fn run(on_refresh: RefreshFn, paths: Paths) -> Result<(), DaemonError> {
    let handle = Handle::current();
    let config = server_config::DaemonServerConfig::from_env();
    let socket_path = paths.socket_path();
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = socket_listener::bind(&paths).await?;

    // 外側プロセスへ起動完了を通知する（stdout pipe 経由）
    {
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"ready\n");
        let _ = std::io::stdout().flush();
    }

    let paths = Arc::new(paths);

    // 旧バージョンのデーモンを非同期でクリーンアップする。
    // 自身のソケットを bind した後に走らせることで、prompt のレスポンスを
    // 旧デーモンの停止完了まで待たせない（生きている旧デーモンには Stop を送信）。
    {
        let cleanup_paths = Arc::clone(&paths);
        tokio::task::spawn_blocking(move || {
            restart::cleanup_old_versions(&cleanup_paths);
        });
    }

    let cancel = CancellationToken::new();
    let state_handle = daemon_state_actor::spawn(config);
    let (exit_tx, mut exit_rx) = tokio_mpsc::unbounded_channel::<()>();

    let mut join_set: JoinSet<()> = JoinSet::new();
    join_set.spawn(scheduler::run(
        state_handle.clone(),
        on_refresh,
        handle.clone(),
        config.scheduler_tick_secs,
        cancel.clone(),
    ));
    if config.rate_limit_aware {
        let interval = Duration::from_secs(config.rate_limit_fetch_interval_secs);
        join_set.spawn(rate_limit_fetcher::run(
            state_handle.clone(),
            Arc::new(RateLimitClient::new(interval)),
            interval,
            cancel.clone(),
        ));
    }
    join_set.spawn(socket_watcher::run(
        Arc::clone(&paths),
        config.socket_check_interval_secs,
        cancel.clone(),
    ));
    join_set.spawn(signals::install_shutdown_signals(cancel.clone()));

    let ctx = AcceptContext {
        state_handle,
        on_refresh,
        exit_tx,
        paths: Arc::clone(&paths),
        handle: handle.clone(),
        cancel: cancel.clone(),
    };
    let should_cleanup = accept_loop(&listener, &ctx, &mut exit_rx).await;

    cancel.cancel();
    while join_set.join_next().await.is_some() {}

    if should_cleanup {
        restart::cleanup(&paths);
    }
    Ok(())
}

/// `accept_loop` の引数をまとめた所有データ。`tokio::spawn` する接続タスクへ
/// `clone` で渡せるよう、各値が独立した所有権を持つ。
struct AcceptContext {
    state_handle: daemon_state_actor::DaemonStateHandle,
    on_refresh: RefreshFn,
    exit_tx: tokio_mpsc::UnboundedSender<()>,
    paths: Arc<Paths>,
    handle: Handle,
    cancel: CancellationToken,
}

async fn accept_loop(
    listener: &tokio::net::UnixListener,
    ctx: &AcceptContext,
    exit_rx: &mut tokio_mpsc::UnboundedReceiver<()>,
) -> bool {
    loop {
        tokio::select! {
            biased;
            _ = exit_rx.recv() => return false,
            () = ctx.cancel.cancelled() => return true,
            accept_result = listener.accept() => match accept_result {
                Ok((s, _)) => {
                    let state_handle = ctx.state_handle.clone();
                    let exit_tx = ctx.exit_tx.clone();
                    let paths = Arc::clone(&ctx.paths);
                    let conn_handle = ctx.handle.clone();
                    let on_refresh = ctx.on_refresh;
                    tokio::spawn(async move {
                        connection::handle(s, &state_handle, on_refresh, &exit_tx, &paths, &conn_handle).await;
                    });
                }
                Err(e) => {
                    log::error!("listener error: {e}");
                    return true;
                }
            }
        }
    }
}

/// リフレッシュ後に `cwd` から `repo_id` を再導出してコールバックを呼ぶ。
/// ブランチが変わっていれば新しい `repo_id` に対してキャッシュを更新する。
pub(super) fn spawn_refresh(
    stored_repo_id: &RepoId,
    cwd: &std::path::Path,
    on_refresh: RefreshFn,
    handle: &Handle,
) {
    let current_repo_id = cwd
        .to_str()
        .and_then(repo_id::repo_id_from_cwd)
        .map_or_else(|| stored_repo_id.clone(), RepoId::new);
    let cwd = cwd.to_path_buf();
    handle.spawn(on_refresh(current_repo_id, cwd));
}
