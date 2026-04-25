//! daemon プロセスを管理するテストヘルパー。

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;

use super::{TestEnv, daemon_dir_name};

/// daemon プロセスを管理するテストヘルパー。
///
/// socket ファイルの出現をポーリングして起動完了を検知する（固定 sleep は使わない）。
/// Drop 時に daemon を停止する。
pub struct DaemonHandle {
    process: std::process::Child,
    pub(super) tmpdir: std::path::PathBuf,
}

impl DaemonHandle {
    pub(super) fn new(process: std::process::Child, tmpdir: std::path::PathBuf) -> Self {
        DaemonHandle { process, tmpdir }
    }
}

impl DaemonHandle {
    /// daemon を起動し、socket が出現するまで最大 2000ms ポーリングする。
    #[must_use]
    pub fn start(env: &TestEnv) -> Self {
        Self::start_with_env(env, &[])
    }

    /// 追加の環境変数を指定して daemon を起動する。
    #[must_use]
    pub fn start_with_env(env: &TestEnv, extra_envs: &[(&str, &str)]) -> Self {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");

        let mut cmd = std::process::Command::new(&bin);
        cmd.args(["daemon", "start"])
            .env("PATH", env.path_env())
            .env("HOME", env.home())
            .env("TMPDIR", env.home())
            .env("XDG_CONFIG_HOME", env.home().join(".config"))
            .current_dir(env.repo_dir.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        for (k, v) in extra_envs {
            cmd.env(k, v);
        }
        let child = cmd.spawn().expect("daemon spawn failed");

        let socket = env
            .home_dir
            .path()
            .join(daemon_dir_name())
            .join("daemon.sock");
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2000);
        while std::time::Instant::now() < deadline {
            if socket.exists() {
                return DaemonHandle::new(child, env.home_dir.path().to_path_buf());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("daemon did not start within 2000ms");
    }

    /// キャッシュに有効な値が入るまで最大 `max_ms` ミリ秒ポーリングする。
    pub fn wait_for_cache(env: &TestEnv, max_ms: u64) {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready-prompt");
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
        loop {
            let out = std::process::Command::new(&bin)
                .env("PATH", env.path_env())
                .env("HOME", env.home())
                .env("TMPDIR", env.home())
                .current_dir(env.repo_dir.path())
                .output()
                .expect("merge-ready-prompt failed");
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout != "? loading" {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "cache not populated within {max_ms}ms"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
        let _ = std::process::Command::new(&bin)
            .args(["daemon", "stop"])
            .env("TMPDIR", &self.tmpdir)
            .output();
        let _ = self.process.kill();
    }
}

/// version 不一致を再現するための簡易 fake daemon。
///
/// `Status` には指定 version を返し、`Query` の version 不一致時には新 daemon を spawn して終了する。
pub struct FakeDaemonHandle {
    join: Option<std::thread::JoinHandle<()>>,
    socket_path: std::path::PathBuf,
}

impl FakeDaemonHandle {
    #[must_use]
    pub fn start_versioned(env: &TestEnv, version: &str) -> Self {
        let socket_path = env.home().join(daemon_dir_name()).join("daemon.sock");
        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent).expect("create fake daemon dir");
        }
        let _ = fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).expect("bind fake daemon socket");
        let version = version.to_owned();
        let socket_path_for_thread = socket_path.clone();
        let tmpdir = env.home().to_path_buf();
        let path_env = env.path_env();
        let join = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    continue;
                };
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
                        // 新 daemon を起動（実際の daemon の自己再起動をシミュレート）
                        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
                        let _ = std::process::Command::new(&bin)
                            .args(["daemon", "start"])
                            .env("TMPDIR", &tmpdir)
                            .env("HOME", &tmpdir)
                            .env("PATH", &path_env)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn();
                        // socket を解放して新 daemon が bind できるようにする
                        let _ = fs::remove_file(&socket_path_for_thread);
                        return;
                    }
                    continue;
                } else {
                    let _ = stream.write_all(b"{\"tag\":\"output\",\"output\":\"? loading\"}\n");
                }
            }
            let _ = fs::remove_file(&socket_path_for_thread);
        });

        Self {
            join: Some(join),
            socket_path,
        }
    }
}

/// JSON 行から client_version フィールドを簡易抽出する。
fn extract_client_version_from_query(line: &str) -> Option<String> {
    let key = "\"client_version\":\"";
    let pos = line.find(key)?;
    let rest = &line[pos + key.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

impl Drop for FakeDaemonHandle {
    fn drop(&mut self) {
        let _ = std::os::unix::net::UnixStream::connect(&self.socket_path)
            .and_then(|mut stream| stream.write_all(b"{\"action\":\"stop\"}\n"));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        let _ = fs::remove_file(&self.socket_path);
    }
}
