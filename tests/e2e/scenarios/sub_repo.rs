//! サブディレクトリシナリオ
//!
//! リポジトリのサブディレクトリで `merge-ready-prompt` を実行したとき、
//! `is_git_repo` が親ディレクトリを遡って `.git` を見つける経路（walk-up）をカバーする。

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const PROMPT_BIN: &str = "merge-ready-prompt";
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// サブディレクトリから実行したとき walk-up で `.git` を検出して通常動作する
#[test]
fn test_prompt_from_subdirectory_finds_parent_git() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    // git root の中にサブディレクトリを作成し、そこを cwd にする
    let sub_dir = env.repo.path().join("sub/dir");
    fs::create_dir_all(&sub_dir).expect("create sub/dir");

    let _daemon = DaemonHandle::start(&env);

    // サブディレクトリから初回実行 → ? loading（バックグラウンドで is_git_repo の walk-up が走る）
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.current_dir(&sub_dir);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // キャッシュが温まるまで待つ
    DaemonHandle::wait_for_cache(&env, 5000);

    // サブディレクトリから再実行 → walk-up で .git を見つけて通常の結果を返す
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.current_dir(&sub_dir);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("✓ Ready for merge"));
}
