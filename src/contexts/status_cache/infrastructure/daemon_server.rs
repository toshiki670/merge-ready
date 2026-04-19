use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::paths;
use super::pid;
use super::protocol::{Request, Response};

const DEFAULT_STALE_TTL_SECS: u64 = 5;
const IDLE_TIMEOUT_SECS: u64 = 30 * 60;

struct CacheEntry {
    output: String,
    fetched_at: Instant,
    refreshing: bool,
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
    stop: bool,
}

/// デーモンのメインループ。ソケットをバインドして接続を待ち受ける。
///
/// この関数は正常には返らない（アイドルタイムアウトまたは Stop リクエストで exit する）。
pub fn run() {
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

    let state = Arc::new(Mutex::new(DaemonState::new()));

    // アイドルタイムアウト監視スレッド
    {
        let state = Arc::clone(&state);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_mins(1));
            let idle = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .last_activity
                .elapsed()
                .as_secs();
            if idle >= IDLE_TIMEOUT_SECS {
                cleanup();
                std::process::exit(0);
            }
        });
    }

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let state = Arc::clone(&state);
                std::thread::spawn(move || handle_client(s, &state));
            }
            Err(_) => break,
        }
    }

    cleanup();
}

fn handle_client(mut stream: std::os::unix::net::UnixStream, state: &Arc<Mutex<DaemonState>>) {
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
        stop,
    } = process(&request, state);

    if let Ok(json) = serde_json::to_string(&response) {
        let _ = stream.write_all(format!("{json}\n").as_bytes());
    }
    drop(stream);

    if let Some(repo_id) = refresh_repo_id {
        spawn_refresh(&repo_id);
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
        Request::Query { repo_id } => {
            match s.entries.get(repo_id) {
                Some(entry) if entry.fetched_at.elapsed().as_secs() <= ttl => ActionResult {
                    response: Response::Fresh {
                        output: entry.output.clone(),
                    },
                    refresh_repo_id: None,
                    stop: false,
                },
                Some(entry) => {
                    let output = entry.output.clone();
                    let need_refresh = !entry.refreshing;
                    if need_refresh {
                        s.entries.get_mut(repo_id).expect("entry exists").refreshing = true;
                    }
                    ActionResult {
                        response: Response::Stale { output },
                        refresh_repo_id: need_refresh.then(|| repo_id.clone()),
                        stop: false,
                    }
                }
                None => {
                    let past = Instant::now()
                        .checked_sub(Duration::from_secs(ttl.saturating_add(1)))
                        .unwrap_or_else(Instant::now);
                    s.entries.insert(
                        repo_id.clone(),
                        CacheEntry {
                            output: String::new(),
                            fetched_at: past,
                            refreshing: true,
                        },
                    );
                    ActionResult {
                        response: Response::Miss,
                        refresh_repo_id: Some(repo_id.clone()),
                        stop: false,
                    }
                }
            }
        }
        Request::Update { repo_id, output } => {
            s.entries.insert(
                repo_id.clone(),
                CacheEntry {
                    output: output.clone(),
                    fetched_at: Instant::now(),
                    refreshing: false,
                },
            );
            ActionResult {
                response: Response::Ok,
                refresh_repo_id: None,
                stop: false,
            }
        }
        Request::Stop => ActionResult {
            response: Response::Ok,
            refresh_repo_id: None,
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
                },
                refresh_repo_id: None,
                stop: false,
            }
        }
    }
}

fn spawn_refresh(repo_id: &str) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(exe)
        .args(["daemon", "refresh", "--repo-id", repo_id])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
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
