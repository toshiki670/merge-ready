use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use tokio::runtime::Handle;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::time::MissedTickBehavior;

use super::connection;
use super::daemon_state_actor::{self, DaemonStateHandle};
use super::paths::Paths;
use super::rate_limit_client::RateLimitClient;
use super::repo_id;
use super::restart;
use super::server_config;
use super::socket_listener;
use crate::contexts::daemon::domain::cache::{Effect, RateLimitObservedEvent, RepoId};
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
        std::thread::spawn(move || {
            restart::cleanup_old_versions(&cleanup_paths);
        });
    }

    let state_handle = daemon_state_actor::spawn(config);
    let (exit_tx, mut exit_rx) = tokio_mpsc::unbounded_channel::<()>();
    let (scheduler_stop_tx, scheduler_stop_rx) = mpsc::channel::<()>();
    let (rate_limit_stop_tx, rate_limit_stop_rx) = mpsc::channel::<()>();

    let scheduler = spawn_scheduler_thread(
        state_handle.clone(),
        on_refresh,
        handle.clone(),
        config.scheduler_tick_secs,
        scheduler_stop_rx,
    );

    let rate_limit_thread: Option<JoinHandle<()>> = if config.rate_limit_aware {
        let interval = Duration::from_secs(config.rate_limit_fetch_interval_secs);
        Some(spawn_rate_limit_fetcher(
            state_handle.clone(),
            Arc::new(RateLimitClient::new(interval)),
            interval,
            rate_limit_stop_rx,
            handle.clone(),
        ))
    } else {
        // OFF 時もスレッドを生やさないが、stop チャネルは drop しても害なし
        drop(rate_limit_stop_rx);
        None
    };

    // 自身の socket ファイル消失を定期チェックする。テストハーネスが異常終了して
    // Drop ベースの `daemon stop` が走らず TempDir ごと消えるケースで孤児化しないように、
    // socket が外部から削除されたら break して自滅する。
    let mut socket_check =
        tokio::time::interval(Duration::from_secs(config.socket_check_interval_secs));
    socket_check.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let should_cleanup = loop {
        tokio::select! {
            biased;
            _ = exit_rx.recv() => break false,
            _ = socket_check.tick() => {
                if !paths.socket_path().exists() {
                    log::info!("daemon socket disappeared, self-terminating");
                    break true;
                }
            }
            accept_result = listener.accept() => match accept_result {
                Ok((s, _)) => {
                    let state_handle = state_handle.clone();
                    let exit_tx = exit_tx.clone();
                    let paths = Arc::clone(&paths);
                    let conn_handle = handle.clone();
                    tokio::spawn(async move {
                        connection::handle(s, &state_handle, on_refresh, &exit_tx, &paths, &conn_handle).await;
                    });
                }
                Err(e) => {
                    log::error!("listener error: {e}");
                    break true;
                }
            }
        }
    };

    let _ = scheduler_stop_tx.send(());
    let _ = rate_limit_stop_tx.send(());
    // std::thread の join は async ワーカーをブロックしないよう block_in_place で
    tokio::task::block_in_place(|| {
        let _ = scheduler.join();
        if let Some(t) = rate_limit_thread {
            let _ = t.join();
        }
    });

    if should_cleanup {
        restart::cleanup(&paths);
    }
    Ok(())
}

/// スケジューラの `std::thread` を起動する。
///
/// `scheduler_tick_secs` 間隔で `state_handle.tick` を呼び、返ってきた `Effect`
/// を drain して `SpawnRefresh` のみを `on_refresh` 経由で tokio タスクとして
/// 起動する。`Phase 4` で `tokio::time::interval` に書き換え予定。
fn spawn_scheduler_thread(
    state_handle: DaemonStateHandle,
    on_refresh: RefreshFn,
    handle: Handle,
    tick_secs: u64,
    stop_rx: mpsc::Receiver<()>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            match stop_rx.recv_timeout(Duration::from_secs(tick_secs)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let effects = handle.block_on(state_handle.tick(Instant::now(), SystemTime::now()));
            for e in effects {
                match e {
                    Effect::SpawnRefresh { repo_id, cwd } => {
                        spawn_refresh(&repo_id, &cwd, on_refresh, &handle);
                    }
                    Effect::RecordExpired { repo_id } => {
                        log::debug!("entry expired: {repo_id:?}");
                    }
                    Effect::EmitOutput(_) | Effect::EnterBackoff { .. } => {}
                }
            }
        }
    })
}

/// `gh api rate_limit` を定期取得して `DaemonState.latest_rate_limit` を更新する。
/// 残量が枯渇／閾値以下まで落ちたとき、`DaemonState.backoff_until` を reset 時刻に
/// セットして daemon 全体のバックグラウンドリフレッシュを停止する。
fn spawn_rate_limit_fetcher(
    state_handle: DaemonStateHandle,
    client: Arc<RateLimitClient>,
    interval: Duration,
    stop_rx: mpsc::Receiver<()>,
    handle: Handle,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            // 初回は起動直後に取得し、その後は `interval` 間隔で繰り返す。
            // async fn `fetch_or_cached` を専用 std::thread から実行するため
            // tokio runtime Handle で block_on する（Phase 4 で async task 化予定）。
            if let Some(snapshot) = handle.block_on(client.fetch_or_cached()) {
                let event = RateLimitObservedEvent {
                    snapshot,
                    now: Instant::now(),
                    now_wall: SystemTime::now(),
                };
                let effects = handle.block_on(state_handle.apply_rate_limit(event));
                for e in effects {
                    if let Effect::EnterBackoff { until } = e {
                        log::info!("rate_limit backoff until {until:?}");
                    }
                }
            }
            match stop_rx.recv_timeout(interval) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    })
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
