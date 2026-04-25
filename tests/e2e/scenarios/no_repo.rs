//! git リポジトリ外シナリオ
//!
//! `.git` のないディレクトリで `merge-ready-prompt` を実行すると即座に空出力を返す。

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const PROMPT_BIN: &str = "merge-ready-prompt";

/// git リポジトリでない場合、何も出力しない
#[test]
fn test_no_git_remote_shows_nothing() {
    let env = TestEnv::without_git_remote();
    // daemon を起動して git リポジトリ外クエリを処理させる
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::is_empty());
}
