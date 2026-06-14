//! daemon のログ出力先パス解決を検証する E2E テスト（シナリオ #52–53）
//!
//! `logger::init()` は daemon 起動時に `<cache>/merge-ready/error.log` を作成する。
//! daemon を起動してファイルの生成先を確認することでパス解決を検証できる。
//!
//! - #52: `XDG_CACHE_HOME` が有効な絶対パス → そこに `merge-ready/error.log` を作る
//! - #53: `XDG_CACHE_HOME` が空文字 → 無効として無視し `$HOME/.cache` にフォールバックする

use std::fs;

use tempfile::tempdir;

use super::super::helpers::{DaemonHandle, TestEnv};

const MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const CHECKS_PASS_JSON: &str =
    r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]"#;

/// #52: `XDG_CACHE_HOME` が有効な絶対パス → そこに `merge-ready/error.log` を作る
#[test]
fn test_xdg_cache_home_is_used_for_log() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    let cache_dir = tempdir().expect("tempdir");

    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[("XDG_CACHE_HOME", cache_dir.path().to_str().unwrap())],
    );

    let log = cache_dir.path().join("merge-ready").join("error.log");
    assert!(
        log.exists(),
        "expected log file under XDG_CACHE_HOME at {}",
        log.display()
    );
}

/// #53: `XDG_CACHE_HOME` が空文字 → 無効として無視し `$HOME/.cache` にフォールバックする
#[test]
fn test_empty_xdg_cache_home_falls_back_to_home() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));

    let _daemon = DaemonHandle::start_with_env(&env, &[("XDG_CACHE_HOME", "")]);

    let log = env
        .home()
        .join(".cache")
        .join("merge-ready")
        .join("error.log");
    assert!(
        log.exists(),
        "expected log file under $HOME/.cache at {}",
        log.display()
    );
}

/// #431: 起動時に既存の `error.log` が上限を超えていたら退避し、active log を有限サイズに戻す。
#[test]
fn test_oversized_error_log_is_rotated_on_start() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    let log = env.home().join(".cache/merge-ready/error.log");
    let dir = log.parent().expect("log dir");
    fs::create_dir_all(dir).expect("create log dir");
    fs::write(&log, "x".repeat(128)).expect("write oversized log");

    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_LOG_MAX_BYTES", "64"),
            ("MERGE_READY_LOG_MAX_BACKUPS", "4"),
        ],
    );

    let backup_lengths: Vec<u64> = (1..=4)
        .filter_map(|index| fs::metadata(dir.join(format!("error.log.{index}"))).ok())
        .map(|meta| meta.len())
        .collect();
    assert!(
        backup_lengths.contains(&128),
        "existing oversized log should be preserved in retained backups, got lengths: {backup_lengths:?}"
    );
    assert!(
        !dir.join("error.log.5").exists(),
        "backup count should not exceed the configured limit"
    );
    assert!(
        fs::metadata(&log).expect("active log metadata").len() <= 64,
        "active error.log should stay within the configured size limit"
    );
}
