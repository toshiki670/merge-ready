// merge-ready-prompt: 軽量なシェルプロンプト用バイナリ。

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

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

    let stream = UnixStream::connect(socket_path()).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)))
        .ok()?;
    let mut stream = stream;

    let msg = encode_query(&cwd, env!("CARGO_PKG_VERSION"));
    stream.write_all(msg.as_bytes()).ok()?;

    let mut buf = String::new();
    BufReader::new(&stream).read_line(&mut buf).ok()?;

    decode_query_response(&buf)
}

/// `{"action":"query","cwd":"...","client_version":"..."}\n`
fn encode_query(cwd: &str, client_version: &str) -> String {
    format!(
        "{}\n",
        serde_json::json!({
            "action": "query",
            "cwd": cwd,
            "client_version": client_version,
        })
    )
}

/// `{"tag":"output","output":"..."}` → output フィールドを返す
fn decode_query_response(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if v.get("tag")?.as_str()? != "output" {
        return None;
    }
    v.get("output")?.as_str().map(str::to_owned)
}

fn socket_path() -> PathBuf {
    std::env::temp_dir().join(dir_name()).join("daemon.sock")
}

fn dir_name() -> String {
    std::cfg_select! {
        target_os = "linux" => {
            use std::os::unix::fs::MetadataExt;
            std::fs::metadata("/proc/self").map_or_else(
                |_| "merge-ready".to_owned(),
                |m| format!("merge-ready-{}", m.uid()),
            )
        },
        _ => "merge-ready".to_owned(),
    }
}

fn spawn_daemon() {
    // 自身のバイナリパス (merge-ready-prompt) と同じディレクトリにある merge-ready を探す
    let daemon_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("merge-ready")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("merge-ready"));

    // fire-and-forget: blocking しない
    let _ = std::process::Command::new(&daemon_exe)
        .args(["daemon", "start"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}
