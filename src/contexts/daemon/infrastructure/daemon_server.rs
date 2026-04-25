use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::paths;
use super::pid;
use super::protocol::{Request, Response};
use super::repo_id;

const DEFAULT_STALE_TTL_SECS: u64 = 5;
const IDLE_TIMEOUT_SECS: u64 = 30 * 60;
const DEFAULT_BACKGROUND_REFRESH_SECS: u64 = 180;
const DEFAULT_REFRESH_LOCK_TIMEOUT_SECS: u64 = 120;
/// version mismatch 後の自己再起動までの待機時間 (ms)
const RESTART_GRACE_MS: u64 = 30;
/// EADDRINUSE 時の bind リトライ間隔
const BIND_RETRY_INTERVAL_MS: u64 = 100;
/// bind リトライ最大回数（合計 1 秒）
const BIND_RETRY_MAX: usize = 10;

struct CacheEntry {
    output: String,
    has_fetched: bool,
    fetched_at: Instant,
    refreshing: bool,
    refresh_started_at: Option<Instant>,
    cwd: PathBuf,
}

struct DaemonState {
    entries: HashMap<String, CacheEntry>,
    last_activity: Instant,
    started_at: Instant,
}

impl DaemonState {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            entries: HashMap::new(),
            last_activity: now,
            started_at: now,
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
/// アイドルタイムアウトまたは Stop リクエストで `ExitCode` を返す。
pub fn run(on_refresh: &RefreshFn) -> ExitCode {
    let socket_path = paths::socket_path();
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = match bind_socket(&socket_path) {
        Ok(l) => l,
        Err(code) => return code,
    };

    pid::write(std::process::id());

    // 外側プロセスへ起動完了を通知する（stdout pipe 経由）
    {
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"ready\n");
        let _ = std::io::stdout().flush();
    }

    let state = Arc::new(Mutex::new(DaemonState::new()));
    let (exit_tx, exit_rx) = mpsc::channel::<ExitCode>();

    // アイドルタイムアウト監視スレッド
    {
        let state = Arc::clone(&state);
        let exit_tx = exit_tx.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_mins(1));
                let guard = state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let has_active_entries = guard.entries.values().any(is_active_entry);
                let idle = guard.last_activity.elapsed().as_secs();
                if has_active_entries {
                    continue;
                }
                if idle >= IDLE_TIMEOUT_SECS {
                    cleanup();
                    let _ = exit_tx.send(ExitCode::SUCCESS);
                    return;
                }
            }
        });
    }

    // 定期バックグラウンドリフレッシュ（prompt 問い合わせがなくても有効PRを更新）
    {
        let state = Arc::clone(&state);
        let on_refresh = Arc::clone(on_refresh);
        std::thread::spawn(move || {
            let interval = background_refresh_secs();
            if interval == 0 {
                return;
            }

            loop {
                std::thread::sleep(Duration::from_secs(interval));
                let refresh_targets = collect_background_refresh_targets(&state, interval);
                for (repo_id, cwd) in refresh_targets {
                    spawn_refresh(&repo_id, &cwd, &on_refresh);
                }
            }
        });
    }

    // non-blocking で accept し、終了シグナルを 10ms ごとにポーリングする
    listener.set_nonblocking(true).ok();

    loop {
        if let Ok(code) = exit_rx.try_recv() {
            return code;
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
    ExitCode::SUCCESS
}

fn bind_socket(socket_path: &std::path::Path) -> Result<UnixListener, ExitCode> {
    // PID ファイルを確認し、生きている daemon が居れば起動しない
    match pid::read() {
        Some(p) if pid::is_alive(p) => {
            log::error!("daemon is already running (pid {p})");
            eprintln!("merge-ready daemon is already running (pid {p})");
            return Err(ExitCode::FAILURE);
        }
        _ => {
            // stale PID または未起動 → socket を安全に削除
            let _ = std::fs::remove_file(socket_path);
        }
    }

    // EADDRINUSE 時は別プロセスが先に bind した → リトライ (version mismatch 再起動レース対策)
    let mut retries = 0;
    loop {
        match UnixListener::bind(socket_path) {
            Ok(l) => return Ok(l),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if retries >= BIND_RETRY_MAX {
                    log::error!("socket already in use after retries, giving up");
                    return Err(ExitCode::SUCCESS);
                }
                retries += 1;
                std::thread::sleep(Duration::from_millis(BIND_RETRY_INTERVAL_MS));
            }
            Err(e) => {
                log::error!("failed to bind socket: {e}");
                eprintln!("merge-ready daemon: failed to bind socket: {e}");
                return Err(ExitCode::FAILURE);
            }
        }
    }
}

fn handle_client(
    mut stream: std::os::unix::net::UnixStream,
    state: &Arc<Mutex<DaemonState>>,
    on_refresh: &RefreshFn,
    exit_tx: &mpsc::Sender<ExitCode>,
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
        // レスポンスが届く時間を確保してから自己再起動する
        std::thread::sleep(Duration::from_millis(RESTART_GRACE_MS));
        cleanup();
        spawn_self_as_daemon();
        let _ = exit_tx.send(ExitCode::SUCCESS);
        return;
    }

    if stop {
        cleanup();
        // レスポンスの書き込みが完了するまで少し待つ
        std::thread::sleep(Duration::from_millis(50));
        let _ = exit_tx.send(ExitCode::SUCCESS);
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
    s.last_activity = Instant::now();

    match request {
        Request::Query {
            cwd,
            client_version,
        } => {
            let version_mismatch = client_version.as_str() != env!("CARGO_PKG_VERSION");

            // repo_id を daemon 側で導出する
            let Some(repo_id) = repo_id::repo_id_from_cwd(cwd) else {
                // git リポジトリ外 → 空文字を返す (PR なし扱い)
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
            process_query(&repo_id, cwd_path, ttl, version_mismatch, &mut s.entries)
        }
        Request::Update { repo_id, output } => process_update(repo_id, output, &mut s.entries),
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
                    pid: std::process::id(),
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
) -> ActionResult {
    match entries.get(repo_id) {
        Some(entry) if entry.fetched_at.elapsed().as_secs() <= ttl => {
            // Fresh キャッシュ
            ActionResult {
                response: Response::Output {
                    output: entry.output.clone(),
                },
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
                restart_after_response,
            }
        }
        Some(entry) => {
            // Stale または Miss（TTL 超過 or 空出力中）
            let output = entry.output.clone();
            let has_fetched = entry.has_fetched;
            let stored_cwd = entry.cwd.clone();
            let need_refresh = !entry.refreshing;
            let refreshing_now = entry.refreshing;
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
            let past = Instant::now()
                .checked_sub(Duration::from_secs(ttl.saturating_add(1)))
                .unwrap_or_else(Instant::now);
            entries.insert(
                repo_id.to_owned(),
                CacheEntry {
                    output: String::new(),
                    has_fetched: false,
                    fetched_at: past,
                    refreshing: true,
                    refresh_started_at: Some(Instant::now()),
                    cwd: cwd_path.clone(),
                },
            );
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

fn spawn_refresh(repo_id: &str, cwd: &std::path::Path, on_refresh: &RefreshFn) {
    let repo_id = repo_id.to_owned();
    let cwd = cwd.to_path_buf();
    let on_refresh = Arc::clone(on_refresh);
    std::thread::spawn(move || on_refresh(&repo_id, &cwd));
}

fn cleanup() {
    let _ = std::fs::remove_file(paths::socket_path());
    pid::remove();
}

fn stale_ttl_secs() -> u64 {
    std::env::var("MERGE_READY_STALE_TTL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_STALE_TTL_SECS)
}

fn background_refresh_secs() -> u64 {
    std::env::var("MERGE_READY_BACKGROUND_REFRESH_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_BACKGROUND_REFRESH_SECS)
}

fn refresh_lock_timeout_secs() -> u64 {
    std::env::var("MERGE_READY_REFRESH_LOCK_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_REFRESH_LOCK_TIMEOUT_SECS)
}

fn mark_refreshing(entry: &mut CacheEntry) {
    entry.refreshing = true;
    entry.refresh_started_at = Some(Instant::now());
}

#[allow(clippy::struct_excessive_bools)]
struct StaleQueryParams {
    output: String,
    has_fetched: bool,
    stored_cwd: PathBuf,
    need_refresh: bool,
    refreshing_now: bool,
    restart_after_response: bool,
}

// no-PR cache is a valid empty output. Keep returning empty output while scheduling background
// refresh, but still show loading for initial placeholder entries that are currently being refreshed.
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
            mark_refreshing(entries.get_mut(repo_id).expect("entry exists"));
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
        mark_refreshing(entries.get_mut(repo_id).expect("entry exists"));
    }
    ActionResult {
        response: Response::Output { output },
        refresh_repo_id: need_refresh.then(|| repo_id.to_owned()),
        refresh_cwd: need_refresh.then_some(stored_cwd),
        stop: false,
        restart_after_response,
    }
}

fn is_active_entry(entry: &CacheEntry) -> bool {
    !entry.output.is_empty()
}

fn refresh_lock_expired(entry: &CacheEntry) -> bool {
    entry
        .refresh_started_at
        .is_some_and(|started| started.elapsed().as_secs() >= refresh_lock_timeout_secs())
}

fn process_update(
    repo_id: &str,
    output: &str,
    entries: &mut HashMap<String, CacheEntry>,
) -> ActionResult {
    if let Some(entry) = entries.get_mut(repo_id) {
        entry.output.clone_from(&output.to_owned());
        entry.has_fetched = true;
        entry.fetched_at = Instant::now();
        entry.refreshing = false;
        entry.refresh_started_at = None;
    } else {
        entries.insert(
            repo_id.to_owned(),
            CacheEntry {
                output: output.to_owned(),
                has_fetched: true,
                fetched_at: Instant::now(),
                refreshing: false,
                refresh_started_at: None,
                cwd: PathBuf::new(),
            },
        );
    }
    ActionResult {
        response: Response::Ok,
        refresh_repo_id: None,
        refresh_cwd: None,
        stop: false,
        restart_after_response: false,
    }
}

fn collect_background_refresh_targets(
    state: &Arc<Mutex<DaemonState>>,
    interval_secs: u64,
) -> Vec<(String, PathBuf)> {
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut targets = Vec::new();
    for (repo_id, entry) in &mut s.entries {
        if !is_active_entry(entry) {
            continue;
        }
        if entry.refreshing && refresh_lock_expired(entry) {
            entry.refreshing = false;
            entry.refresh_started_at = None;
        }
        if entry.refreshing {
            continue;
        }
        if entry.fetched_at.elapsed().as_secs() < interval_secs {
            continue;
        }
        mark_refreshing(entry);
        targets.push((repo_id.clone(), entry.cwd.clone()));
    }

    if !targets.is_empty() {
        // 有効PRの定期更新が動いている間は daemon を生かす
        s.last_activity = Instant::now();
    }
    targets
}
