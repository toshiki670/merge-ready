//! git リポジトリ外シナリオ
//!
//! `.git` のないディレクトリで `prompt` を実行すると即座に空出力を返す。

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::TestEnv;

const BIN: &str = "merge-ready";

/// git リポジトリでない場合、何も出力しない
#[test]
fn test_no_git_remote_shows_nothing() {
    let env = TestEnv::without_git_remote();

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::is_empty());
}
