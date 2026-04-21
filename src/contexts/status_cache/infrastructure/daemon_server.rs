use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::paths;
use super::pid;
use super::protocol::{Request, Response};

const DEFAULT_STALE_TTL_SECS: u64 = 5;
const IDLE_TIMEOUT_SECS: u64 = 30 * 60;
const DEFAULT_BACKGROUND_REFRESH_SECS: u64 = 180;
const DEFAULT_REFRESH_LOCK_TIMEOUT_SECS: u64 = 120;

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
}

type RefreshFn = Arc<dyn Fn(&str, &std::path::Path) + Send + Sync + 'static>;

/// デーモンのメインループ。ソケットをバインドして接続を待ち受ける。
///
/// `on_refresh` はキャッシュ更新が必要になったときにスレッドで呼ばれる。
/// この関数は正常には返らない（アイドルタイムアウトまたは Stop リクエストで exit する）。
pub fn run(on_refresh: &RefreshFn) {
    let socket_path = paths::socket_path();
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // 前回の残留ソケットを除去してからバインド
    let _ = std::fs::remove_file(&socket_path);

    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("merge-ready daemon: failed to bind socket: {e}");
            std::process::exit(1);
        }
    };

    pid::write(std::process::id());

    // 外側プロセスへ起動完了を通知する（stdout pipe 経由）
    {
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"ready\n");
        let _ = std::io::stdout().flush();
    }

    let state = Arc::new(Mutex::new(DaemonState::new()));

    // アイドルタイムアウト監視スレッド
    {
        let state = Arc::clone(&state);
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
                    std::process::exit(0);
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

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let state = Arc::clone(&state);
                let on_refresh = Arc::clone(on_refresh);
                std::thread::spawn(move || handle_client(s, &state, &on_refresh));
            }
            Err(_) => break,
        }
    }

    cleanup();
}

fn handle_client(
    mut stream: std::os::unix::net::UnixStream,
    state: &Arc<Mutex<DaemonState>>,
    on_refresh: &RefreshFn,
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
    } = process(&request, state);

    if let Ok(json) = serde_json::to_string(&response) {
        let _ = stream.write_all(format!("{json}\n").as_bytes());
    }
    drop(stream);

    if let (Some(repo_id), Some(cwd)) = (refresh_repo_id, refresh_cwd) {
        spawn_refresh(&repo_id, &cwd, on_refresh);
    }

    if stop {
        cleanup();
        // レスポンスの書き込みが完了するまで少し待つ
        std::thread::sleep(Duration::from_millis(50));
        std::process::exit(0);
    }
}

fn process(request: &Request, state: &Arc<Mutex<DaemonState>>) -> ActionResult {
    let ttl = stale_ttl_secs();
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    s.last_activity = Instant::now();

    match request {
        Request::Query { repo_id, cwd } => {
            let cwd_path = PathBuf::from(cwd);
            match s.entries.get(repo_id) {
                Some(entry) if entry.fetched_at.elapsed().as_secs() <= ttl => ActionResult {
                    response: Response::Fresh {
                        output: entry.output.clone(),
                    },
                    refresh_repo_id: None,
                    refresh_cwd: None,
                    stop: false,
                },
                Some(entry) => {
                    let output = entry.output.clone();
                    let has_fetched = entry.has_fetched;
                    let stored_cwd = entry.cwd.clone();
                    let need_refresh = !entry.refreshing;
                    let refreshing_now = entry.refreshing;
                    process_stale_query(
                        repo_id,
                        output,
                        has_fetched,
                        stored_cwd,
                        need_refresh,
                        refreshing_now,
                        &mut s.entries,
                    )
                }
                None => {
                    let past = Instant::now()
                        .checked_sub(Duration::from_secs(ttl.saturating_add(1)))
                        .unwrap_or_else(Instant::now);
                    s.entries.insert(
                        repo_id.clone(),
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
                        response: Response::Miss,
                        refresh_repo_id: Some(repo_id.clone()),
                        refresh_cwd: Some(cwd_path),
                        stop: false,
                    }
                }
            }
        }
        Request::Update { repo_id, output } => process_update(repo_id, output, &mut s.entries),
        Request::Stop => ActionResult {
            response: Response::Ok,
            refresh_repo_id: None,
            refresh_cwd: None,
            stop: true,
        },
        Request::Status => {
            let uptime_secs = s.started_at.elapsed().as_secs();
            let entries = s.entries.len();
            ActionResult {
                response: Response::Status {
                    pid: std::process::id(),
                    entries,
                    uptime_secs,
                    version: daemon_version(),
                },
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
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

// no-PR cache is a valid empty output. Keep returning empty output while scheduling background
// refresh, but still show loading for initial placeholder entries that are currently being refreshed.
fn process_stale_query(
    repo_id: &str,
    output: String,
    has_fetched: bool,
    stored_cwd: PathBuf,
    need_refresh: bool,
    refreshing_now: bool,
    entries: &mut HashMap<String, CacheEntry>,
) -> ActionResult {
    if output.is_empty() {
        if refreshing_now && !has_fetched {
            return ActionResult {
                response: Response::Miss,
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
            };
        }
        if need_refresh {
            mark_refreshing(entries.get_mut(repo_id).expect("entry exists"));
        }
        return ActionResult {
            response: Response::Fresh {
                output: String::new(),
            },
            refresh_repo_id: need_refresh.then(|| repo_id.to_owned()),
            refresh_cwd: need_refresh.then_some(stored_cwd),
            stop: false,
        };
    }

    if need_refresh {
        mark_refreshing(entries.get_mut(repo_id).expect("entry exists"));
    }
    ActionResult {
        response: Response::Stale { output },
        refresh_repo_id: need_refresh.then(|| repo_id.to_owned()),
        refresh_cwd: need_refresh.then_some(stored_cwd),
        stop: false,
    }
}

fn is_active_entry(entry: &CacheEntry) -> bool {
    // no-PR entries are intentionally treated as inactive so they do not keep
    // the daemon alive forever. They can be re-created on the next prompt.
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

fn daemon_version() -> String {
    // Test-only override to emulate an older daemon binary.
    std::env::var("MERGE_READY_DAEMON_VERSION_OVERRIDE")
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_owned())
}
