//! デフォルトブランチ上シナリオ: main / master どちらのデフォルトブランチでも空出力
//!
//! デフォルトブランチ上では PR が存在しないため何も出力しない。
//! `is_default_branch()` が main / master を動的に取得して正しく判定することを検証する。

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

/// main ブランチ（デフォルトブランチ）上: daemon キャッシュ確定後に空出力
#[test]
fn test_daemon_on_main_default_branch_outputs_empty() {
    let env = TestEnv::with_default_branch_no_pr("main");
    let _daemon = DaemonHandle::start(&env);

    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(""))
        .stderr("");
}

/// master ブランチ（デフォルトブランチ）上: daemon キャッシュ確定後に空出力
#[test]
fn test_daemon_on_master_default_branch_outputs_empty() {
    let env = TestEnv::with_default_branch_no_pr("master");
    let _daemon = DaemonHandle::start(&env);

    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(""))
        .stderr("");
}
