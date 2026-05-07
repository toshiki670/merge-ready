use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{FakeDaemonHandle, TestEnv, daemon_dir_name};

const BIN: &str = "merge-ready";
const PROMPT_BIN: &str = "merge-ready-prompt";
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

fn assert_prompt_succeeded(output: &std::process::Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "prompt failed: status={:?}, stdout={stdout:?}, stderr={stderr:?}",
        output.status.code(),
    );
    assert!(
        stderr.is_empty(),
        "prompt stderr should be empty: {stderr:?}"
    );
    assert!(
        stdout == "? loading" || stdout == "✓ Ready for merge",
        "unexpected prompt stdout: {stdout:?}",
    );
}

fn run_prompts_concurrently(env: &TestEnv, count: usize) {
    let prompt_bin = assert_cmd::cargo::cargo_bin(PROMPT_BIN);
    let handles: Vec<_> = (0..count)
        .map(|_| {
            let bin = prompt_bin.clone();
            let path = env.path_env();
            let home = env.home().to_path_buf();
            let repo = env.repo_dir.path().to_path_buf();
            std::thread::spawn(move || {
                std::process::Command::new(&bin)
                    .env("PATH", &path)
                    .env("HOME", &home)
                    .env("TMPDIR", &home)
                    .current_dir(&repo)
                    .output()
            })
        })
        .collect();

    for h in handles {
        let output = h
            .join()
            .expect("prompt thread panicked")
            .expect("run prompt");
        assert_prompt_succeeded(&output);
    }
}

fn socket_path(env: &TestEnv) -> std::path::PathBuf {
    env.home_dir
        .path()
        .join(daemon_dir_name())
        .join("daemon.sock")
}

fn wait_for_current_daemon_version(env: &TestEnv) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(5000);
    loop {
        let out = Command::cargo_bin(BIN)
            .unwrap()
            .args(["daemon", "status"])
            .env("PATH", env.path_env())
            .env("HOME", env.home())
            .env("TMPDIR", env.home())
            .output()
            .expect("status failed");
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.contains(&format!("version={}", env!("CARGO_PKG_VERSION"))) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "new daemon did not start within 5s: {stdout}"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// #15: 複数の `merge-ready-prompt` が同時に Daemon 起動を試みても、Daemon は 1 プロセスのみ存在する
#[test]
fn test_concurrent_prompt_starts_only_one_daemon() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    run_prompts_concurrently(&env, 20);

    let mut status = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut status);
    status
        .args(["daemon", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("running"));

    assert!(socket_path(&env).exists(), "daemon socket should exist");

    Command::cargo_bin(BIN)
        .unwrap()
        .args(["daemon", "stop"])
        .env("TMPDIR", env.home())
        .output()
        .ok();
}

/// #16: バージョン不一致の状態で複数の `merge-ready-prompt` を並列実行しても、
/// 新デーモンは 1 プロセスだけ起動する
#[test]
fn test_concurrent_version_mismatch_starts_only_one_daemon() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _old = FakeDaemonHandle::start_versioned(&env, "0.0.0");

    run_prompts_concurrently(&env, 10);
    wait_for_current_daemon_version(&env);

    assert!(socket_path(&env).exists(), "daemon socket should exist");

    let mut after = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut after);
    after
        .args(["daemon", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "version={}",
            env!("CARGO_PKG_VERSION")
        )))
        .stdout(predicate::str::contains("version=0.0.0").not());

    Command::cargo_bin(BIN)
        .unwrap()
        .args(["daemon", "stop"])
        .env("TMPDIR", env.home())
        .output()
        .ok();
}
