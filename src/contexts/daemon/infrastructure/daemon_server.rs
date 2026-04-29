use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::paths;
use super::pid;
use super::protocol::{Request, Response};
use super::repo_id;
use crate::contexts::daemon::domain::cache::{CacheEntry, RefreshMode};
use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;

const DEFAULT_STALE_TTL_SECS: u64 = 5;
const DEFAULT_REFRESH_LOCK_TIMEOUT_SECS: u64 = 120;
/// version mismatch 後の自己再起動までの待機時間 (ms)
const RESTART_GRACE_MS: u64 = 30;
/// EADDRINUSE 時の bind リトライ間隔
const BIND_RETRY_INTERVAL_MS: u64 = 100;
/// bind リトライ最大回数（合計 1 秒）
const BIND_RETRY_MAX: usize = 10;
/// バックグラウンドスケジューラの動作間隔。Hot 最小間隔（2 秒）に合わせる。
const SCHEDULER_TICK_SECS: u64 = 2;

// ── Hot モード ────────────────────────────────────────────────────────────────
/// 「最近 Query あり」と見なす経過秒数
const DEFAULT_HOT_RECENT_QUERY_SECS: u64 = 30;
/// Hot + 最近 Query あり の場合のリフレッシュ間隔
const DEFAULT_HOT_WITH_QUERY_SECS: u64 = 2;
/// Hot のみ（Query なし）の場合のリフレッシュ間隔
const DEFAULT_HOT_WITHOUT_QUERY_SECS: u64 = 10;

// ── Warm モード ───────────────────────────────────────────────────────────────
const DEFAULT_WARM_REFRESH_SECS: u64 = 180;
/// Warm から Cold へ移行するまでの Query 無し経過秒数
const DEFAULT_WARM_TO_COLD_SECS: u64 = 30 * 60;

// ── Cold モード ───────────────────────────────────────────────────────────────
/// Cold 初期（累計リフレッシュ `COLD_EARLY_LIMIT` 回まで）の間隔
const DEFAULT_COLD_EARLY_SECS: u64 = 30 * 60;
/// Cold 後期（`COLD_EARLY_LIMIT` 回超）の間隔
const DEFAULT_COLD_LATE_SECS: u64 = 60 * 60;
/// Cold 初期から後期へ切り替わる累計リフレッシュ回数
const DEFAULT_COLD_EARLY_LIMIT: u32 = 10;

// ── エントリ寿命 ──────────────────────────────────────────────────────────────
/// 最終 Query から この秒数が経過したエントリを削除する（2 日）
const DEFAULT_ENTRY_MAX_AGE_SECS: u64 = 2 * 24 * 60 * 60;

struct DaemonState {
    entries: HashMap<String, CacheEntry>,
    started_at: Instant,
    policy: RefreshPolicy,
}

impl DaemonState {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            started_at: Instant::now(),
            policy: RefreshPolicy {
                hot_recent_query_secs: hot_recent_query_secs(),
                hot_with_query_secs: hot_with_query_secs(),
                hot_without_query_secs: hot_without_query_secs(),
                warm_refresh_secs: warm_refresh_secs(),
                warm_to_cold_secs: warm_to_cold_secs(),
                cold_early_secs: cold_early_secs(),
                cold_late_secs: cold_late_secs(),
                cold_early_limit: cold_early_limit(),
            },
        }
    }
}

struct ActionResult {
    response: Response,
    /// `Some(repo_id)` のとき、ロック解放後にリフレッシュを起動する
    refresh_repo_id: Option<String>,
    refresh_cwd: Option<PathBuf>,
    stop: bool,
    /// レスポンス返却後に自己再起動する（version mismatch 時）
    restart_after_response: bool,
}

type RefreshFn = Arc<dyn Fn(&str, &std::path::Path) + Send + Sync + 'static>;

/// デーモンのメインループ。ソケットをバインドして接続を待ち受ける。
///
/// `on_refresh` はキャッシュ更新が必要になったときにスレッドで呼ばれる。
/// Stop リクエストで `Ok(())` を返す。
pub fn run(on_refresh: &RefreshFn) -> Result<(), ()> {
    let socket_path = paths::socket_path();
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = bind_socket(&socket_path)?;

    pid::write(std::process::id());

    // 外側プロセスへ起動完了を通知する（stdout pipe 経由）
    {
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"ready\n");
        let _ = std::io::stdout().flush();
    }

    let state = Arc::new(Mutex::new(DaemonState::new()));
    let (exit_tx, exit_rx) = mpsc::channel::<()>();

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
                std::thread::spawn(move || handle_client(s, &state, &on_refresh, &exit_tx));
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

fn bind_socket(socket_path: &std::path::Path) -> Result<UnixListener, ()> {
    match pid::read() {
        Some(p) if pid::is_alive(p) => {
            log::error!("daemon is already running (pid {p})");
            eprintln!("merge-ready daemon is already running (pid {p})");
            return Err(());
        }
        _ => {
            let _ = std::fs::remove_file(socket_path);
        }
    }

    let mut retries = 0;
    loop {
        match UnixListener::bind(socket_path) {
            Ok(l) => return Ok(l),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if retries >= BIND_RETRY_MAX {
                    log::error!("socket already in use after retries, giving up");
                    return Err(());
                }
                retries += 1;
                std::thread::sleep(Duration::from_millis(BIND_RETRY_INTERVAL_MS));
            }
            Err(e) => {
                log::error!("failed to bind socket: {e}");
                eprintln!("merge-ready daemon: failed to bind socket: {e}");
                return Err(());
            }
        }
    }
}

fn handle_client(
    mut stream: std::os::unix::net::UnixStream,
    state: &Arc<Mutex<DaemonState>>,
    on_refresh: &RefreshFn,
    exit_tx: &mpsc::Sender<()>,
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
    } = process(&request, state);

    if let Ok(json) = serde_json::to_string(&response) {
        let _ = stream.write_all(format!("{json}\n").as_bytes());
    }
    drop(stream);

    if let (Some(repo_id), Some(cwd)) = (refresh_repo_id, refresh_cwd) {
        spawn_refresh(&repo_id, &cwd, on_refresh);
    }

    if restart_after_response {
        std::thread::sleep(Duration::from_millis(RESTART_GRACE_MS));
        cleanup();
        spawn_self_as_daemon();
        let _ = exit_tx.send(());
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
    let _ = std::process::Command::new(&exe)
        .args(["daemon", "start"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn process(request: &Request, state: &Arc<Mutex<DaemonState>>) -> ActionResult {
    let ttl = stale_ttl_secs();
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let policy = s.policy;

    match request {
        Request::Query {
            cwd,
            client_version,
        } => {
            let version_mismatch = client_version.as_str() != env!("CARGO_PKG_VERSION");

            let Some(repo_id) = repo_id::repo_id_from_cwd(cwd) else {
                return ActionResult {
                    response: Response::Output {
                        output: String::new(),
                    },
                    refresh_repo_id: None,
                    refresh_cwd: None,
                    stop: false,
                    restart_after_response: version_mismatch,
                };
            };

            let cwd_path = PathBuf::from(cwd);
            process_query(
                &repo_id,
                cwd_path,
                ttl,
                version_mismatch,
                &mut s.entries,
                &policy,
            )
        }
        Request::Update {
            repo_id,
            output,
            refresh_mode,
        } => process_update(repo_id, output, *refresh_mode, &mut s.entries),
        Request::Stop => ActionResult {
            response: Response::Ok,
            refresh_repo_id: None,
            refresh_cwd: None,
            stop: true,
            restart_after_response: false,
        },
        Request::Status => {
            let uptime_secs = s.started_at.elapsed().as_secs();
            let entries = s.entries.len();
            ActionResult {
                response: Response::Status {
                    entries,
                    uptime_secs,
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                },
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
                restart_after_response: false,
            }
        }
    }
}

fn process_query(
    repo_id: &str,
    cwd_path: PathBuf,
    ttl: u64,
    restart_after_response: bool,
    entries: &mut HashMap<String, CacheEntry>,
    policy: &RefreshPolicy,
) -> ActionResult {
    match entries.get_mut(repo_id) {
        Some(entry) if entry.is_fresh(policy.effective_ttl(entry, ttl)) => {
            // Fresh キャッシュ
            entry.record_query();
            ActionResult {
                response: Response::Output {
                    output: entry.output().to_owned(),
                },
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
                restart_after_response,
            }
        }
        Some(entry) => {
            // Stale または TTL 超過
            let output = entry.output().to_owned();
            let has_fetched = entry.has_fetched();
            let stored_cwd = entry.cwd().to_path_buf();
            let need_refresh = !entry.is_refreshing();
            let refreshing_now = entry.is_refreshing();
            let is_terminal = entry.refresh_mode() == RefreshMode::Terminal;
            // Query を受けたので last_queried_at を更新し Cold カウンタをリセット
            let was_cold = entry.is_cold_or_never_queried(policy.warm_to_cold_secs);
            if was_cold {
                entry.reset_cold_count();
            }
            entry.record_query();
            if is_terminal && need_refresh {
                // Terminal が stale になったらモードをリセットして再確認
                entry.reset_to_warm();
            }
            process_stale_query(
                repo_id,
                StaleQueryParams {
                    output,
                    has_fetched,
                    stored_cwd,
                    need_refresh,
                    refreshing_now,
                    restart_after_response,
                },
                entries,
            )
        }
        None => {
            // 初回 Miss → エントリを作成してリフレッシュ予約
            entries.insert(repo_id.to_owned(), CacheEntry::new(cwd_path.clone(), ttl));
            ActionResult {
                response: Response::Output {
                    output: "? loading".to_owned(),
                },
                refresh_repo_id: Some(repo_id.to_owned()),
                refresh_cwd: Some(cwd_path),
                stop: false,
                restart_after_response,
            }
        }
    }
}

/// リフレッシュ後に `cwd` から `repo_id` を再導出してコールバックを呼ぶ。
/// ブランチが変わっていれば新しい `repo_id` に対してキャッシュを更新する。
fn spawn_refresh(stored_repo_id: &str, cwd: &std::path::Path, on_refresh: &RefreshFn) {
    let current_repo_id = cwd
        .to_str()
        .and_then(repo_id::repo_id_from_cwd)
        .unwrap_or_else(|| stored_repo_id.to_owned());
    let cwd = cwd.to_path_buf();
    let on_refresh = Arc::clone(on_refresh);
    std::thread::spawn(move || on_refresh(&current_repo_id, &cwd));
}

fn cleanup() {
    let _ = std::fs::remove_file(paths::socket_path());
    pid::remove();
}

// ── 設定値（環境変数オーバーライド対応） ─────────────────────────────────────

fn stale_ttl_secs() -> u64 {
    env_u64("MERGE_READY_STALE_TTL", DEFAULT_STALE_TTL_SECS)
}

fn refresh_lock_timeout_secs() -> u64 {
    env_u64(
        "MERGE_READY_REFRESH_LOCK_TIMEOUT_SECS",
        DEFAULT_REFRESH_LOCK_TIMEOUT_SECS,
    )
}

fn hot_recent_query_secs() -> u64 {
    env_u64(
        "MERGE_READY_HOT_RECENT_QUERY_SECS",
        DEFAULT_HOT_RECENT_QUERY_SECS,
    )
}

fn hot_with_query_secs() -> u64 {
    env_u64(
        "MERGE_READY_HOT_WITH_QUERY_SECS",
        DEFAULT_HOT_WITH_QUERY_SECS,
    )
}

fn hot_without_query_secs() -> u64 {
    env_u64(
        "MERGE_READY_HOT_WITHOUT_QUERY_SECS",
        DEFAULT_HOT_WITHOUT_QUERY_SECS,
    )
}

fn warm_refresh_secs() -> u64 {
    env_u64("MERGE_READY_WARM_REFRESH_SECS", DEFAULT_WARM_REFRESH_SECS)
}

fn warm_to_cold_secs() -> u64 {
    env_u64("MERGE_READY_WARM_TO_COLD_SECS", DEFAULT_WARM_TO_COLD_SECS)
}

fn cold_early_secs() -> u64 {
    env_u64("MERGE_READY_COLD_EARLY_SECS", DEFAULT_COLD_EARLY_SECS)
}

fn cold_late_secs() -> u64 {
    env_u64("MERGE_READY_COLD_LATE_SECS", DEFAULT_COLD_LATE_SECS)
}

fn cold_early_limit() -> u32 {
    std::env::var("MERGE_READY_COLD_EARLY_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_COLD_EARLY_LIMIT)
}

fn entry_max_age_secs() -> u64 {
    env_u64("MERGE_READY_ENTRY_MAX_AGE_SECS", DEFAULT_ENTRY_MAX_AGE_SECS)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

// ── 内部処理ヘルパー ──────────────────────────────────────────────────────────

#[allow(clippy::struct_excessive_bools)]
struct StaleQueryParams {
    output: String,
    has_fetched: bool,
    stored_cwd: PathBuf,
    need_refresh: bool,
    refreshing_now: bool,
    restart_after_response: bool,
}

fn process_stale_query(
    repo_id: &str,
    params: StaleQueryParams,
    entries: &mut HashMap<String, CacheEntry>,
) -> ActionResult {
    let StaleQueryParams {
        output,
        has_fetched,
        stored_cwd,
        need_refresh,
        refreshing_now,
        restart_after_response,
    } = params;

    if output.is_empty() {
        if refreshing_now && !has_fetched {
            return ActionResult {
                response: Response::Output {
                    output: "? loading".to_owned(),
                },
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
                restart_after_response,
            };
        }
        if need_refresh {
            entries
                .get_mut(repo_id)
                .expect("entry exists")
                .mark_refreshing();
        }
        return ActionResult {
            response: Response::Output {
                output: String::new(),
            },
            refresh_repo_id: need_refresh.then(|| repo_id.to_owned()),
            refresh_cwd: need_refresh.then_some(stored_cwd),
            stop: false,
            restart_after_response,
        };
    }

    if need_refresh {
        entries
            .get_mut(repo_id)
            .expect("entry exists")
            .mark_refreshing();
    }
    ActionResult {
        response: Response::Output { output },
        refresh_repo_id: need_refresh.then(|| repo_id.to_owned()),
        refresh_cwd: need_refresh.then_some(stored_cwd),
        stop: false,
        restart_after_response,
    }
}

/// エントリは必ず `process_query`（Query 経由）で生成される。
/// 未知の `repo_id` への Update（ブランチ切替直後の再導出 ID など）は無視する。
/// `cwd: PathBuf::new()` / `last_queried_at: None` の孤立エントリが生まれるのを防ぐ。
fn process_update(
    repo_id: &str,
    output: &str,
    refresh_mode: RefreshMode,
    entries: &mut HashMap<String, CacheEntry>,
) -> ActionResult {
    if let Some(entry) = entries.get_mut(repo_id) {
        entry.update(output.to_owned(), refresh_mode);
    }
    ActionResult {
        response: Response::Ok,
        refresh_repo_id: None,
        refresh_cwd: None,
        stop: false,
        restart_after_response: false,
    }
}

fn collect_background_refresh_targets(state: &Arc<Mutex<DaemonState>>) -> Vec<(String, PathBuf)> {
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let policy = s.policy;

    // 期限切れエントリを削除
    s.entries
        .retain(|_, entry| !entry.is_expired(entry_max_age_secs()));

    let mut targets = Vec::new();
    for (repo_id, entry) in &mut s.entries {
        if !entry.is_active() {
            continue;
        }
        if entry.is_refreshing() && entry.refresh_lock_expired(refresh_lock_timeout_secs()) {
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
        let mut e = CacheEntry::new(PathBuf::new(), 5);
        e.update(output.to_owned(), refresh_mode);
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

    // ── is_active (via CacheEntry) ─────────────────────────────────────────────

    #[test]
    fn active_when_non_empty_output_and_warm() {
        assert!(make_entry("✓ Ready for merge", RefreshMode::Warm).is_active());
    }

    #[test]
    fn active_when_hot() {
        assert!(make_entry("⧖ Wait for CI", RefreshMode::Hot).is_active());
    }

    #[test]
    fn inactive_when_empty_output() {
        assert!(!make_entry("", RefreshMode::Warm).is_active());
    }

    #[test]
    fn inactive_when_terminal() {
        assert!(!make_entry("✓ Ready for merge", RefreshMode::Terminal).is_active());
    }

    // ── process_update ─────────────────────────────────────────────────────────

    #[test]
    fn process_update_sets_refresh_mode_terminal() {
        let mut entries = HashMap::new();
        entries.insert(
            "repo".to_owned(),
            make_entry("✓ Ready for merge", RefreshMode::Warm),
        );
        process_update("repo", "", RefreshMode::Terminal, &mut entries);
        assert_eq!(entries["repo"].refresh_mode(), RefreshMode::Terminal);
    }

    #[test]
    fn process_update_sets_refresh_mode_hot() {
        let mut entries = HashMap::new();
        entries.insert(
            "repo".to_owned(),
            make_entry("✓ Ready for merge", RefreshMode::Warm),
        );
        process_update("repo", "⧖ Wait for CI", RefreshMode::Hot, &mut entries);
        assert_eq!(entries["repo"].refresh_mode(), RefreshMode::Hot);
    }

    #[test]
    fn process_update_unknown_repo_id_is_ignored() {
        // ブランチ切替後に spawn_refresh が新 repo_id で Update してきた場合、
        // エントリを新規作成せず無視する（孤立エントリ防止）
        let mut entries = HashMap::new();
        process_update("unknown-repo", "output", RefreshMode::Warm, &mut entries);
        assert!(
            entries.is_empty(),
            "未知の repo_id への Update はエントリを作成しないはず"
        );
    }

    #[test]
    fn process_update_clears_terminal_when_pr_reopens() {
        let mut entries = HashMap::new();
        entries.insert("repo".to_owned(), make_entry("", RefreshMode::Terminal));
        process_update("repo", "✓ Ready for merge", RefreshMode::Warm, &mut entries);
        assert_eq!(entries["repo"].refresh_mode(), RefreshMode::Warm);
    }

    // ── collect_background_refresh_targets ─────────────────────────────────────

    #[test]
    fn background_refresh_skips_terminal_entry() {
        let state = Arc::new(Mutex::new(DaemonState::new()));
        {
            let mut s = state.lock().unwrap();
            s.entries.insert(
                "repo".to_owned(),
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
            s.entries.insert("repo".to_owned(), entry);
        }
        let targets = collect_background_refresh_targets(&state);
        assert_eq!(targets.len(), 1);
    }

    #[test]
    fn background_refresh_increments_cold_count() {
        let state = Arc::new(Mutex::new(DaemonState::new()));
        {
            let mut s = state.lock().unwrap();
            let mut entry = make_stale_entry("✓ Ready for merge", RefreshMode::Warm, 9999);
            entry.last_queried_at = Some(
                Instant::now()
                    .checked_sub(Duration::from_secs(warm_to_cold_secs() + 1))
                    .unwrap(),
            );
            entry.cold_refresh_count = 3;
            entry.cwd = PathBuf::from("/some/repo");
            s.entries.insert("repo".to_owned(), entry);
        }
        collect_background_refresh_targets(&state);
        let s = state.lock().unwrap();
        assert_eq!(s.entries["repo"].cold_refresh_count(), 4);
    }

    #[test]
    fn background_refresh_removes_expired_entries() {
        let state = Arc::new(Mutex::new(DaemonState::new()));
        {
            let mut s = state.lock().unwrap();
            let mut entry = make_entry("✓ Ready for merge", RefreshMode::Warm);
            entry.last_queried_at = Some(
                Instant::now()
                    .checked_sub(Duration::from_secs(entry_max_age_secs() + 1))
                    .unwrap(),
            );
            s.entries.insert("repo".to_owned(), entry);
        }
        collect_background_refresh_targets(&state);
        let s = state.lock().unwrap();
        assert!(s.entries.is_empty(), "期限切れエントリは削除されるはず");
    }
}
