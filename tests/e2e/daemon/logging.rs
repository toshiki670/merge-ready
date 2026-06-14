//! daemon のログ出力先パス解決を検証する E2E テスト（シナリオ #52–53）
//!
//! `logger::init()` は daemon 起動時に `<cache>/merge-ready/error.log` を作成する。
//! daemon を起動してファイルの生成先を確認することでパス解決を検証できる。
//!
//! - #52: `XDG_CACHE_HOME` が有効な絶対パス → そこに `merge-ready/error.log` を作る
//! - #53: `XDG_CACHE_HOME` が空文字 → 無効として無視し `$HOME/.cache` にフォールバックする

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
