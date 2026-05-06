//! キャッシュライフサイクル シナリオ #1: 初回ロード
//!
//! daemon 未起動 → `? loading` → daemon 自動起動 → キャッシュが温まる → 結果表示

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const BIN: &str = "merge-ready";
const PROMPT_BIN: &str = "merge-ready-prompt";
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// daemon 未起動 → `? loading`（daemon を自動起動）→ キャッシュ温まる → 結果表示
#[test]
fn test_initial_load_then_shows_result() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    // daemon なし → ? loading（バックグラウンドで daemon が自動起動される）
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // daemon がキャッシュを温めるまで待つ
    DaemonHandle::wait_for_cache(&env, 5000);

    // キャッシュヒット → 実際の結果を返す
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("✓ Ready for merge"));

    // 自動起動された daemon を後始末
    let bin = assert_cmd::cargo::cargo_bin(BIN);
    std::process::Command::new(&bin)
        .args(["daemon", "stop"])
        .env("TMPDIR", env.home())
        .output()
        .ok();
}

/// daemon 起動直後（キャッシュなし）→ `? loading`
#[test]
fn test_daemon_miss_shows_loading() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));
}
