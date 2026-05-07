use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{FakeDaemonHandle, TestEnv};

const BIN: &str = "merge-ready";
const PROMPT_BIN: &str = "merge-ready-prompt";
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

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

/// #14: バージョン不一致の旧 daemon が存在する状態で `merge-ready-prompt` を実行すると
/// 旧 daemon がレスポンス返却後に自己再起動し、最終的に現バージョンの daemon が応答する
#[test]
fn test_prompt_restarts_daemon_on_version_mismatch() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let old = FakeDaemonHandle::start_versioned(&env, "0.0.0");

    let mut before = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut before);
    before
        .args(["daemon", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("version=0.0.0"));

    let mut prompt = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut prompt);
    prompt.assert().success();

    drop(old);
    wait_for_current_daemon_version(&env);

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
