//! GitHub 非対応リポジトリシナリオ
//!
//! git リポジトリだが GitHub に接続されていない場合、何も表示しない。

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

const PROMPT_BIN: &str = "merge-ready-prompt";

/// remote 未設定 / GitHub 以外の remote では何も表示しない
#[rstest]
#[case::no_remotes("no git remotes found")]
#[case::non_github_remote(
    "none of the git remotes configured for this repository point to a known GitHub host. \
     To tell gh about a new GitHub host, please use `gh auth login`"
)]
fn test_non_github_repo_shows_nothing(#[case] stderr: &str) {
    let env = TestEnv::with_error(stderr, 1);
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr("");
}
