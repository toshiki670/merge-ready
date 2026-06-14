//! `config_path()` の環境変数によるパス解決を検証する E2E テスト（シナリオ #49–51）
//!
//! - #49: `XDG_CONFIG_HOME` が設定されている → そちらの設定ファイルを優先する
//! - #50: `XDG_CONFIG_HOME` と `HOME` 両方ある → `XDG_CONFIG_HOME` が勝つ
//! - #51: `XDG_CONFIG_HOME` が空文字 → 無効として無視し `$HOME/.config` にフォールバックする

use assert_cmd::Command;

use super::super::helpers::{DaemonHandle, TestEnv};

const MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const CHECKS_PASS_JSON: &str =
    r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]"#;

const PROMPT_BIN: &str = "merge-ready-prompt";

/// #49: `XDG_CONFIG_HOME` が設定されている → そちらの設定ファイルを読む
#[test]
fn test_xdg_config_home_is_used() {
    use std::fs;
    use tempfile::tempdir;

    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    let xdg_dir = tempdir().expect("tempdir");
    fs::write(
        xdg_dir.path().join("merge-ready.toml"),
        "[merge_ready]\nsymbol = \"★\"",
    )
    .expect("write config");

    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[("XDG_CONFIG_HOME", xdg_dir.path().to_str().unwrap())],
    );

    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout("★ Ready for merge")
        .stderr("");
}

/// #51: `XDG_CONFIG_HOME` が空文字 → 無効として無視し `$HOME/.config` を読む。
///
/// XDG Base Directory 仕様では空の値は未設定として扱う。空文字を `Some` として
/// 受け入れると設定パスがカレントディレクトリ相対の `merge-ready.toml` に化けるため、
/// 確実に `$HOME/.config` 側の設定が読まれることを検証する。
#[test]
fn test_empty_xdg_config_home_falls_back_to_home() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    // HOME 側にデフォルト (`✓`) とは異なるシンボルを置く。空の XDG_CONFIG_HOME が
    // 無視されればこれが読まれ、相対パスへ化けてデフォルトになることはない。
    env.write_config("[merge_ready]\nsymbol = \"★\"");

    let _daemon = DaemonHandle::start_with_env(&env, &[("XDG_CONFIG_HOME", "")]);

    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout("★ Ready for merge")
        .stderr("");
}

/// #50: `XDG_CONFIG_HOME` と `HOME` 両方ある → `XDG_CONFIG_HOME` が優先される
#[test]
fn test_xdg_config_home_takes_precedence_over_home() {
    use std::fs;
    use tempfile::tempdir;

    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    // HOME 側にも設定を置く（XDG 側が優先されるはず）
    env.write_config("[merge_ready]\nsymbol = \"✓\"");

    let xdg_dir = tempdir().expect("tempdir");
    fs::write(
        xdg_dir.path().join("merge-ready.toml"),
        "[merge_ready]\nsymbol = \"★\"",
    )
    .expect("write xdg config");

    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[("XDG_CONFIG_HOME", xdg_dir.path().to_str().unwrap())],
    );

    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout("★ Ready for merge")
        .stderr("");
}
