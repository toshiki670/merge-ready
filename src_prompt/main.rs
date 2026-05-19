// merge-ready-prompt: 軽量なシェルプロンプト用バイナリ。

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::time::Duration;

use merge_ready::{prompt_ipc, prompt_socket_path};

const READ_TIMEOUT_MS: u64 = 500;

fn main() {
    let output = query_daemon().unwrap_or_else(|| {
        // 接続失敗 → daemon を非同期起動して "? loading" を返す
        spawn_daemon();
        "? loading".to_owned()
    });
    print!("{output}");
}

fn query_daemon() -> Option<String> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    let stream = UnixStream::connect(prompt_socket_path()).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)))
        .ok()?;
    let mut stream = stream;

    let req = prompt_ipc::Request { cwd };
    stream.write_all(req.encode().as_bytes()).ok()?;

    // レスポンスはスタックバッファで受け取る（8KB BufReader ヒープ確保を回避）
    let mut buf = [0u8; 512];
    let n = stream.read(&mut buf).ok()?;

    prompt_ipc::Response::decode(&buf[..n]).map(|r| r.output)
}

fn spawn_daemon() {
    // 自身のバイナリパス (merge-ready-prompt) と同じディレクトリにある merge-ready を探す。
    // current_exe / parent は実用上失敗しない前提で expect する。
    let exe = std::env::current_exe().expect("current_exe");
    let daemon_exe = exe.parent().expect("exe parent").join("merge-ready");

    // MERGE_READY_DAEMON_INNER=1 で outer wrapper をスキップして直接 inner として起動する。
    // この文字列は infrastructure::paths::DAEMON_INNER_ENV と同一でなければならない。
    // fire-and-forget: blocking しない
    let _ = std::process::Command::new(&daemon_exe)
        .args(["daemon", "start"])
        .env("MERGE_READY_DAEMON_INNER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}
