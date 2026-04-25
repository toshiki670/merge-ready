// merge-ready-prompt: 軽量なシェルプロンプト用バイナリ。
// std のみ使用。外部クレートの依存を持たない。

mod protocol {
    include!("../protocol/protocol.rs");
}

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

    let msg = protocol::encode_query(&cwd, env!("CARGO_PKG_VERSION"));
    stream.write_all(msg.as_bytes()).ok()?;

    let mut buf = String::new();
    BufReader::new(&stream).read_line(&mut buf).ok()?;

    protocol::decode_query_response(&buf)
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
