use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use super::background_refresh;
use super::connection;
use super::paths::Paths;
use super::rate_limit_client::RateLimitClient;
use super::repo_id;
use super::restart;
use super::server_config;
use super::server_state::DaemonState;
use super::socket_listener;
use crate::contexts::daemon::domain::cache::RepoId;
use crate::contexts::daemon::domain::daemon::DaemonError;

/// ボトルネック残量比率がこの値（basis points）以下になったとき、
/// reset 時刻まで backoff に入る。
const BACKOFF_THRESHOLD_BP: u64 = 500; // 5%

pub(super) type RefreshFn = Arc<dyn Fn(&RepoId, &std::path::Path) + Send + Sync + 'static>;

/// デーモンのメインループ。ソケットをバインドして接続を待ち受ける。
///
/// `on_refresh` はキャッシュ更新が必要になったときにスレッドで呼ばれる。
/// Stop リクエストで `Ok(())` を返す。
pub fn run(on_refresh: &RefreshFn, paths: Paths) -> Result<(), DaemonError> {
    let config = server_config::DaemonServerConfig::from_env();
    let socket_path = paths.socket_path();
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = socket_listener::bind(&paths)?;

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

    let state = Arc::new(Mutex::new(DaemonState::new(config)));
    let (exit_tx, exit_rx) = mpsc::channel::<()>();
    let (scheduler_stop_tx, scheduler_stop_rx) = mpsc::channel::<()>();
    let (rate_limit_stop_tx, rate_limit_stop_rx) = mpsc::channel::<()>();

    let scheduler = {
        let state = Arc::clone(&state);
        let on_refresh = Arc::clone(on_refresh);
        std::thread::spawn(move || {
            loop {
                match scheduler_stop_rx
                    .recv_timeout(Duration::from_secs(config.scheduler_tick_secs))
                {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                let refresh_targets = background_refresh::collect_targets(&state);
                for (repo_id, cwd) in refresh_targets {
                    spawn_refresh(&repo_id, &cwd, &on_refresh);
                }
            }
        })
    };

    let rate_limit_thread: Option<JoinHandle<()>> = if config.rate_limit_aware {
        let interval = Duration::from_secs(config.rate_limit_fetch_interval_secs);
        Some(spawn_rate_limit_fetcher(
            Arc::clone(&state),
            Arc::new(RateLimitClient::new(interval)),
            interval,
            rate_limit_stop_rx,
        ))
    } else {
        // OFF 時もスレッドを生やさないが、stop チャネルは drop しても害なし
        drop(rate_limit_stop_rx);
        None
    };

    // non-blocking で accept し、終了シグナルを 10ms ごとにポーリングする
    listener.set_nonblocking(true).ok();

    // 自身の socket ファイル消失を定期チェックする。テストハーネスが異常終了して
    // Drop ベースの `daemon stop` が走らず TempDir ごと消えるケースで孤児化しないように、
    // socket が外部から削除されたら break して自滅する。
    let socket_check_interval = Duration::from_secs(config.socket_check_interval_secs);
    let mut last_socket_check = Instant::now();

    let should_cleanup = loop {
        if exit_rx.try_recv().is_ok() {
            break false;
        }

        if last_socket_check.elapsed() >= socket_check_interval {
            last_socket_check = Instant::now();
            if !paths.socket_path().exists() {
                log::info!("daemon socket disappeared, self-terminating");
                break true;
            }
        }

        match listener.accept() {
            Ok((s, _)) => {
                let state = Arc::clone(&state);
                let on_refresh = Arc::clone(on_refresh);
                let exit_tx = exit_tx.clone();
                let paths = Arc::clone(&paths);
                std::thread::spawn(move || {
                    connection::handle(s, &state, &on_refresh, &exit_tx, &paths);
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                log::error!("listener error: {e}");
                break true;
            }
        }
    };

    let _ = scheduler_stop_tx.send(());
    let _ = scheduler.join();
    let _ = rate_limit_stop_tx.send(());
    if let Some(t) = rate_limit_thread {
        let _ = t.join();
    }

    if should_cleanup {
        restart::cleanup(&paths);
    }
    Ok(())
}

/// `gh api rate_limit` を定期取得して `DaemonState.latest_rate_limit` を更新する。
/// 残量が枯渇／閾値以下まで落ちたとき、`DaemonState.backoff_until` を reset 時刻に
/// セットして daemon 全体のバックグラウンドリフレッシュを停止する。
fn spawn_rate_limit_fetcher(
    state: Arc<Mutex<DaemonState>>,
    client: Arc<RateLimitClient>,
    interval: Duration,
    stop_rx: mpsc::Receiver<()>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            // 初回は起動直後に取得し、その後は `interval` 間隔で繰り返す
            if let Some(snapshot) = client.fetch_or_cached() {
                update_state_from_snapshot(&state, &snapshot);
            }
            match stop_rx.recv_timeout(interval) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    })
}

fn update_state_from_snapshot(
    state: &Arc<Mutex<DaemonState>>,
    snapshot: &crate::contexts::daemon::domain::rate_limit_snapshot::RateLimitSnapshot,
) {
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    s.latest_rate_limit = Some(*snapshot);

    // ボトルネック残量が閾値以下なら reset 時刻まで backoff
    if should_enter_backoff(snapshot)
        && let Some(reset_instant) = reset_instant_from_snapshot(snapshot)
    {
        s.set_backoff(reset_instant);
    }
}

fn should_enter_backoff(
    snapshot: &crate::contexts::daemon::domain::rate_limit_snapshot::RateLimitSnapshot,
) -> bool {
    if snapshot.is_exhausted() {
        return true;
    }
    let core_bp = ratio_bp(snapshot.core_remaining, snapshot.core_limit);
    let graphql_bp = ratio_bp(snapshot.graphql_remaining, snapshot.graphql_limit);
    core_bp.min(graphql_bp) <= BACKOFF_THRESHOLD_BP
}

fn ratio_bp(remaining: u32, limit: u32) -> u64 {
    if limit == 0 {
        return 10_000;
    }
    (u64::from(remaining).saturating_mul(10_000)) / u64::from(limit)
}

/// `snapshot.reset_at`（壁時計）を `Instant`（モノトニック）へ変換する。
/// `SystemTime` のジャンプにロバストにするため、`now_wall` と `now_instant` の差分で換算する。
fn reset_instant_from_snapshot(
    snapshot: &crate::contexts::daemon::domain::rate_limit_snapshot::RateLimitSnapshot,
) -> Option<Instant> {
    let now_wall = SystemTime::now();
    let now_instant = Instant::now();
    let delta = snapshot.reset_at.duration_since(now_wall).ok()?;
    Some(now_instant + delta)
}

/// リフレッシュ後に `cwd` から `repo_id` を再導出してコールバックを呼ぶ。
/// ブランチが変わっていれば新しい `repo_id` に対してキャッシュを更新する。
pub(super) fn spawn_refresh(
    stored_repo_id: &RepoId,
    cwd: &std::path::Path,
    on_refresh: &RefreshFn,
) {
    let current_repo_id = cwd
        .to_str()
        .and_then(repo_id::repo_id_from_cwd)
        .map_or_else(|| stored_repo_id.clone(), RepoId::new);
    let cwd = cwd.to_path_buf();
    let on_refresh = Arc::clone(on_refresh);
    std::thread::spawn(move || on_refresh(&current_repo_id, &cwd));
}
