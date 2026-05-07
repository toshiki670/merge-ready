use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::background_refresh;
use super::connection;
use super::paths;
use super::repo_id;
use super::restart;
use super::server_config;
use super::server_state::DaemonState;
use super::socket_listener;
use crate::contexts::daemon::domain::cache::RepoId;
use crate::contexts::daemon::domain::daemon::DaemonError;

pub(super) type RefreshFn = Arc<dyn Fn(&RepoId, &std::path::Path) + Send + Sync + 'static>;

/// デーモンのメインループ。ソケットをバインドして接続を待ち受ける。
///
/// `on_refresh` はキャッシュ更新が必要になったときにスレッドで呼ばれる。
/// Stop リクエストで `Ok(())` を返す。
pub fn run(on_refresh: &RefreshFn) -> Result<(), DaemonError> {
    let config = server_config::DaemonServerConfig::from_env();
    let socket_path = paths::socket_path();
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = socket_listener::bind(&socket_path)?;

    // 外側プロセスへ起動完了を通知する（stdout pipe 経由）
    {
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"ready\n");
        let _ = std::io::stdout().flush();
    }

    let state = Arc::new(Mutex::new(DaemonState::new(config)));
    let (exit_tx, exit_rx) = mpsc::channel::<()>();
    let restart_started = Arc::new(AtomicBool::new(false));

    // 定期バックグラウンドリフレッシュ
    // SCHEDULER_TICK_SECS ごとに各エントリのリフレッシュ間隔を個別に評価する
    {
        let state = Arc::clone(&state);
        let on_refresh = Arc::clone(on_refresh);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(config.scheduler_tick_secs));
                let refresh_targets = background_refresh::collect_targets(&state);
                for (repo_id, cwd) in refresh_targets {
                    spawn_refresh(&repo_id, &cwd, &on_refresh);
                }
            }
        });
    }

    // non-blocking で accept し、終了シグナルを 10ms ごとにポーリングする
    listener.set_nonblocking(true).ok();

    loop {
        if exit_rx.try_recv().is_ok() {
            return Ok(());
        }

        match listener.accept() {
            Ok((s, _)) => {
                let state = Arc::clone(&state);
                let on_refresh = Arc::clone(on_refresh);
                let exit_tx = exit_tx.clone();
                let restart_started = Arc::clone(&restart_started);
                std::thread::spawn(move || {
                    connection::handle(s, &state, &on_refresh, &exit_tx, &restart_started);
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                log::error!("listener error: {e}");
                break;
            }
        }
    }

    restart::cleanup();
    Ok(())
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
