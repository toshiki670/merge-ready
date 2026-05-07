use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::paths;
use super::pid;
use super::protocol::Request;
use super::repo_id;
use super::request_handler::{self, ActionResult};
use super::server_config;
use crate::contexts::daemon::domain::cache::{CacheEntry, RefreshMode, RepoId};
use crate::contexts::daemon::domain::daemon::DaemonError;
use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;

/// version mismatch 後の自己再起動までの待機時間 (ms)
const RESTART_GRACE_MS: u64 = 30;
/// EADDRINUSE 時の bind リトライ間隔
const BIND_RETRY_INTERVAL_MS: u64 = 100;
/// bind リトライ最大回数（合計 1 秒）
const BIND_RETRY_MAX: usize = 10;
/// バックグラウンドスケジューラの動作間隔。Hot 最小間隔（2 秒）に合わせる。
const SCHEDULER_TICK_SECS: u64 = 2;

struct DaemonState {
    entries: HashMap<RepoId, CacheEntry>,
    started_at: Instant,
    policy: RefreshPolicy,
}

impl DaemonState {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            started_at: Instant::now(),
            policy: RefreshPolicy {
                hot_recent_query_secs: server_config::hot_recent_query_secs(),
                hot_with_query_secs: server_config::hot_with_query_secs(),
                hot_without_query_secs: server_config::hot_without_query_secs(),
                warm_refresh_secs: server_config::warm_refresh_secs(),
                warm_to_cold_secs: server_config::warm_to_cold_secs(),
                cold_early_secs: server_config::cold_early_secs(),
                cold_late_secs: server_config::cold_late_secs(),
                cold_early_limit: server_config::cold_early_limit(),
            },
        }
    }
}

type RefreshFn = Arc<dyn Fn(&RepoId, &std::path::Path) + Send + Sync + 'static>;

/// デーモンのメインループ。ソケットをバインドして接続を待ち受ける。
///
/// `on_refresh` はキャッシュ更新が必要になったときにスレッドで呼ばれる。
/// Stop リクエストで `Ok(())` を返す。
pub fn run(on_refresh: &RefreshFn) -> Result<(), DaemonError> {
    let socket_path = paths::socket_path();
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = bind_socket(&socket_path)?;

    // 外側プロセスへ起動完了を通知する（stdout pipe 経由）
    {
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"ready\n");
        let _ = std::io::stdout().flush();
    }

    let state = Arc::new(Mutex::new(DaemonState::new()));
    let (exit_tx, exit_rx) = mpsc::channel::<()>();
    let restart_started = Arc::new(AtomicBool::new(false));

    // 定期バックグラウンドリフレッシュ
    // SCHEDULER_TICK_SECS ごとに各エントリのリフレッシュ間隔を個別に評価する
    {
        let state = Arc::clone(&state);
        let on_refresh = Arc::clone(on_refresh);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(SCHEDULER_TICK_SECS));
                let refresh_targets = collect_background_refresh_targets(&state);
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
                    handle_client(s, &state, &on_refresh, &exit_tx, &restart_started);
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

    cleanup();
    Ok(())
}

fn bind_socket(socket_path: &std::path::Path) -> Result<UnixListener, DaemonError> {
    let startup_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(paths::lock_path())
        .map_err(|e| {
            log::error!("failed to open daemon lock file: {e}");
            eprintln!("merge-ready daemon: failed to open lock file: {e}");
            DaemonError::Failure
        })?;
    startup_lock.lock().map_err(|e| {
        log::error!("failed to lock daemon startup: {e}");
        eprintln!("merge-ready daemon: failed to acquire startup lock: {e}");
        DaemonError::Failure
    })?;

    match pid::read() {
        Some(p) if pid::is_alive(p) => {
            log::error!("daemon is already running (pid {p})");
            eprintln!("merge-ready daemon is already running (pid {p})");
            return Err(DaemonError::AlreadyRunning);
        }
        Some(_) => {
            pid::remove();
            let _ = std::fs::remove_file(socket_path);
        }
        None => {
            let _ = std::fs::remove_file(socket_path);
        }
    }

    let mut retries = 0;
    loop {
        match UnixListener::bind(socket_path) {
            Ok(l) => {
                pid::write(std::process::id());
                return Ok(l);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if retries >= BIND_RETRY_MAX {
                    log::error!("socket already in use after retries, giving up");
                    return Err(DaemonError::AlreadyRunning);
                }
                retries += 1;
                std::thread::sleep(Duration::from_millis(BIND_RETRY_INTERVAL_MS));
            }
            Err(e) => {
                log::error!("failed to bind socket: {e}");
                eprintln!("merge-ready daemon: failed to bind socket: {e}");
                return Err(DaemonError::Failure);
            }
        }
    }
}

fn handle_client(
    mut stream: std::os::unix::net::UnixStream,
    state: &Arc<Mutex<DaemonState>>,
    on_refresh: &RefreshFn,
    exit_tx: &mpsc::Sender<()>,
    restart_started: &Arc<AtomicBool>,
) {
    let mut buf = String::new();
    {
        let mut reader = BufReader::new(&stream);
        if reader.read_line(&mut buf).is_err() || buf.is_empty() {
            return;
        }
    }

    let request: Request = match serde_json::from_str(buf.trim()) {
        Ok(r) => r,
        Err(_) => return,
    };

    let ActionResult {
        response,
        refresh_repo_id,
        refresh_cwd,
        stop,
        restart_after_response,
    } = {
        let ttl = server_config::stale_ttl_secs();
        let mut s = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let policy = s.policy;
        let started_at = s.started_at;
        request_handler::process(&request, &mut s.entries, &policy, started_at, ttl)
    };

    if let Ok(json) = serde_json::to_string(&response) {
        let _ = stream.write_all(format!("{json}\n").as_bytes());
    }
    drop(stream);

    if let (Some(repo_id), Some(cwd)) = (refresh_repo_id, refresh_cwd) {
        spawn_refresh(&repo_id, &cwd, on_refresh);
    }

    if restart_after_response {
        if restart_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            std::thread::sleep(Duration::from_millis(RESTART_GRACE_MS));
            cleanup();
            spawn_self_as_daemon();
            let _ = exit_tx.send(());
        }
        return;
    }

    if stop {
        cleanup();
        std::thread::sleep(Duration::from_millis(50));
        let _ = exit_tx.send(());
    }
}

fn spawn_self_as_daemon() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    // DAEMON_INNER_ENV を設定して outer wrapper をスキップし、直接 inner として起動する。
    let _ = std::process::Command::new(&exe)
        .args(["daemon", "start"])
        .env(paths::DAEMON_INNER_ENV, "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// リフレッシュ後に `cwd` から `repo_id` を再導出してコールバックを呼ぶ。
/// ブランチが変わっていれば新しい `repo_id` に対してキャッシュを更新する。
fn spawn_refresh(stored_repo_id: &RepoId, cwd: &std::path::Path, on_refresh: &RefreshFn) {
    let current_repo_id = cwd
        .to_str()
        .and_then(repo_id::repo_id_from_cwd)
        .map_or_else(|| stored_repo_id.clone(), RepoId::new);
    let cwd = cwd.to_path_buf();
    let on_refresh = Arc::clone(on_refresh);
    std::thread::spawn(move || on_refresh(&current_repo_id, &cwd));
}

fn cleanup() {
    let _ = std::fs::remove_file(paths::socket_path());
    pid::remove();
}

fn collect_background_refresh_targets(state: &Arc<Mutex<DaemonState>>) -> Vec<(RepoId, PathBuf)> {
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let policy = s.policy;

    // 期限切れエントリを削除
    s.entries
        .retain(|_, entry| !entry.is_expired(server_config::entry_max_age_secs()));

    let mut targets = Vec::new();
    for (repo_id, entry) in &mut s.entries {
        if !entry.is_active() {
            continue;
        }
        if entry.is_refreshing()
            && entry.refresh_lock_expired(server_config::refresh_lock_timeout_secs())
        {
            entry.clear_refresh_lock();
        }
        if entry.is_refreshing() {
            continue;
        }
        let interval = policy.effective_refresh_interval_secs(entry);
        if entry.fetched_at.elapsed().as_secs() < interval {
            continue;
        }
        // Cold モードでリフレッシュする場合はカウンタを進める
        if entry.refresh_mode() == RefreshMode::Warm && entry.is_cold(policy.warm_to_cold_secs) {
            entry.increment_cold_count();
        }
        entry.mark_refreshing();
        targets.push((repo_id.clone(), entry.cwd.clone()));
    }

    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(output: &str, refresh_mode: RefreshMode) -> CacheEntry {
        let mut e = CacheEntry::new(PathBuf::new(), String::new(), 5);
        e.update(output.to_owned(), vec![], refresh_mode);
        e.record_query();
        e
    }

    fn make_stale_entry(output: &str, refresh_mode: RefreshMode, age_secs: u64) -> CacheEntry {
        let mut e = make_entry(output, refresh_mode);
        e.fetched_at = Instant::now()
            .checked_sub(Duration::from_secs(age_secs))
            .unwrap_or_else(Instant::now);
        e
    }

    // ── collect_background_refresh_targets ─────────────────────────────────────

    #[test]
    fn background_refresh_skips_terminal_entry() {
        let state = Arc::new(Mutex::new(DaemonState::new()));
        {
            let mut s = state.lock().unwrap();
            s.entries.insert(
                RepoId::new("repo"),
                make_stale_entry("✓ Ready for merge", RefreshMode::Terminal, 9999),
            );
        }
        let targets = collect_background_refresh_targets(&state);
        assert!(
            targets.is_empty(),
            "Terminal エントリはバックグラウンドリフレッシュ対象外のはず"
        );
    }

    #[test]
    fn background_refresh_includes_stale_hot_entry() {
        let state = Arc::new(Mutex::new(DaemonState::new()));
        {
            let mut s = state.lock().unwrap();
            let mut entry = make_stale_entry("⧖ Wait for CI", RefreshMode::Hot, 9999);
            entry.cwd = PathBuf::from("/some/repo");
            s.entries.insert(RepoId::new("repo"), entry);
        }
        let targets = collect_background_refresh_targets(&state);
        assert_eq!(targets.len(), 1);
    }

    #[test]
    fn background_refresh_increments_cold_count() {
        let repo_id = RepoId::new("repo");
        let state = Arc::new(Mutex::new(DaemonState::new()));
        {
            let mut s = state.lock().unwrap();
            let mut entry = make_stale_entry("✓ Ready for merge", RefreshMode::Warm, 9999);
            entry.last_queried_at = Some(
                Instant::now()
                    .checked_sub(Duration::from_secs(server_config::warm_to_cold_secs() + 1))
                    .unwrap(),
            );
            entry.cold_refresh_count = 3;
            entry.cwd = PathBuf::from("/some/repo");
            s.entries.insert(repo_id.clone(), entry);
        }
        collect_background_refresh_targets(&state);
        let s = state.lock().unwrap();
        assert_eq!(s.entries[&repo_id].cold_refresh_count(), 4);
    }

    // ── restart_started AtomicBool ─────────────────────────────────────────────

    #[test]
    fn restart_executes_only_once_under_concurrent_threads() {
        use std::sync::atomic::AtomicU32;
        let restart_started = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicU32::new(0));
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let rs = Arc::clone(&restart_started);
                let c = Arc::clone(&count);
                std::thread::spawn(move || {
                    if rs
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        c.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(
            count.load(Ordering::Relaxed),
            1,
            "再起動は1回だけ実行されるはず"
        );
    }

    #[test]
    fn background_refresh_removes_expired_entries() {
        let state = Arc::new(Mutex::new(DaemonState::new()));
        {
            let mut s = state.lock().unwrap();
            let mut entry = make_entry("✓ Ready for merge", RefreshMode::Warm);
            entry.last_queried_at = Some(
                Instant::now()
                    .checked_sub(Duration::from_secs(server_config::entry_max_age_secs() + 1))
                    .unwrap(),
            );
            s.entries.insert(RepoId::new("repo"), entry);
        }
        collect_background_refresh_targets(&state);
        let s = state.lock().unwrap();
        assert!(s.entries.is_empty(), "期限切れエントリは削除されるはず");
    }
}
