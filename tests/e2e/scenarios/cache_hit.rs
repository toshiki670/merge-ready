//! キャッシュライフサイクル シナリオ #2: キャッシュヒット
//!
//! daemon + キャッシュ済み → `merge-ready-prompt` が即座に結果を返す

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const PROMPT_BIN: &str = "merge-ready-prompt";
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// daemon 起動 → キャッシュ温まる → 壊れた gh でも daemon キャッシュからヒット
#[test]
fn test_daemon_fresh_returns_cached_output() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    // 初回: キャッシュミス → daemon が内部でリフレッシュを実行
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // キャッシュが温まるまで待つ
    DaemonHandle::wait_for_cache(&env, 5000);

    // 壊れた gh を使っても daemon キャッシュからヒットすること
    let broken_env = TestEnv::with_error("gh is broken", 1);
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    cmd.env("PATH", broken_env.path_env());
    cmd.env("HOME", env.home());
    cmd.env("TMPDIR", env.home());
    cmd.current_dir(env.repo_dir.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("✓ Ready for merge"));
}
