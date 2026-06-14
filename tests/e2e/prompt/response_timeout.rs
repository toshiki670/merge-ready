//! `merge-ready-prompt` の daemon 応答読み取り全体のタイムアウトを検証する。

const PROMPT_BIN: &str = "merge-ready-prompt";
const PROMPT_TIMEOUT_MS: u64 = 1_500;

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use super::super::helpers::{TestEnv, apply_coverage_env};

fn versioned_socket(base: &std::path::Path) -> std::path::PathBuf {
    base.join(format!("daemon-{}.sock", env!("CARGO_PKG_VERSION")))
}

fn run_prompt_with_timeout(env: &TestEnv) -> std::process::Output {
    let bin = assert_cmd::cargo::cargo_bin(PROMPT_BIN);
    let mut cmd = std::process::Command::new(bin);
    cmd.env("PATH", env.path_env())
        .env("HOME", env.home())
        .env("TMPDIR", env.home())
        .env("MERGE_READY_BASE_DIR", env.home())
        .current_dir(env.repo.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    apply_coverage_env(&mut cmd);

    let mut child = cmd.spawn().expect("spawn merge-ready-prompt");
    let deadline = Instant::now() + Duration::from_millis(PROMPT_TIMEOUT_MS);
    loop {
        if child.try_wait().is_ok_and(|s| s.is_some()) {
            return child.wait_with_output().expect("collect prompt output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("merge-ready-prompt did not finish within {PROMPT_TIMEOUT_MS}ms");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn slow_drip_response_falls_back_to_loading_at_total_deadline() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#,
        Some(r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]"#),
    );
    let socket_path = versioned_socket(env.home());
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path).expect("bind fake daemon socket");

    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept prompt connection");
        let mut request = Vec::new();
        let mut buf = [0_u8; 64];
        loop {
            let n = stream.read(&mut buf).expect("read prompt request");
            if n == 0 {
                return;
            }
            request.extend_from_slice(&buf[..n]);
            if request.contains(&b'\n') {
                break;
            }
        }

        for byte in br#"{"tag":"output","output":"this response never reaches newline""# {
            if stream.write_all(&[*byte]).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    });

    let output = run_prompt_with_timeout(&env);
    server.join().expect("fake daemon thread");

    assert!(
        output.status.success(),
        "prompt failed: status={:?}, stdout={:?}, stderr={:?}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "? loading");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}
