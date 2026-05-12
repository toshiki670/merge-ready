use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::mpsc;

use super::super::helpers::TestEnv;

/// version 不一致を再現するための簡易 fake daemon。
///
/// `Status` には指定 version を返し、`Query` の version 不一致時には新 daemon を spawn して終了する。
pub struct FakeDaemonHandle {
    join: Option<std::thread::JoinHandle<()>>,
    stop_tx: Option<mpsc::Sender<()>>,
}

impl FakeDaemonHandle {
    #[must_use]
    pub fn start_versioned(env: &TestEnv, version: &str) -> Self {
        let socket_path = env.home().join("daemon.sock");
        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent).expect("create fake daemon dir");
        }
        let _ = fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).expect("bind fake daemon socket");
        listener
            .set_nonblocking(true)
            .expect("set fake daemon nonblocking");
        let version = version.to_owned();
        let socket_path_for_thread = socket_path.clone();
        let tmpdir = env.home().to_path_buf();
        let path_env = env.path_env();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let join = std::thread::spawn(move || {
            let mut remove_socket_on_exit = true;
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => continue,
                };
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
                let mut line = String::new();
                {
                    let mut reader = BufReader::new(&stream);
                    if reader.read_line(&mut line).is_err() {
                        continue;
                    }
                }
                if line.contains("\"action\":\"status\"") {
                    let _ = stream.write_all(
                        format!(
                            "{{\"tag\":\"status\",\"pid\":1,\"entries\":0,\"uptime_secs\":0,\"version\":\"{version}\"}}\n"
                        )
                        .as_bytes(),
                    );
                } else if line.contains("\"action\":\"stop\"") {
                    let _ = stream.write_all(b"{\"tag\":\"ok\"}\n");
                    break;
                } else if line.contains("\"action\":\"query\"") {
                    // Query に対して "? loading" を返す
                    let _ = stream.write_all(b"{\"tag\":\"output\",\"output\":\"? loading\"}\n");
                    drop(stream);

                    // クライアントのバージョンが自身と異なる場合は自己再起動をシミュレート
                    let client_version =
                        extract_client_version_from_query(&line).unwrap_or_default();
                    if client_version != version {
                        // socket を解放して新 daemon が bind できるようにする
                        let _ = fs::remove_file(&socket_path_for_thread);
                        remove_socket_on_exit = false;

                        // 新 daemon を起動（実際の daemon の自己再起動をシミュレート）
                        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
                        let _ = std::process::Command::new(&bin)
                            .args(["daemon", "start"])
                            .env("TMPDIR", &tmpdir)
                            .env("HOME", &tmpdir)
                            .env("MERGE_READY_BASE_DIR", &tmpdir)
                            .env("PATH", &path_env)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn();
                        break;
                    }
                } else {
                    let _ = stream.write_all(b"{\"tag\":\"output\",\"output\":\"? loading\"}\n");
                }
            }
            if remove_socket_on_exit {
                let _ = fs::remove_file(&socket_path_for_thread);
            }
        });

        Self {
            join: Some(join),
            stop_tx: Some(stop_tx),
        }
    }
}

/// JSON 行から `client_version` フィールドを簡易抽出する。
fn extract_client_version_from_query(line: &str) -> Option<String> {
    let key = "\"client_version\":\"";
    let pos = line.find(key)?;
    let rest = &line[pos + key.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

impl Drop for FakeDaemonHandle {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
