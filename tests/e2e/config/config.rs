//! 設定ファイル（`~/.config/merge-ready.toml`）の E2E テスト（シナリオ #42–58）
//!
//! - #42–50: symbol / label / format のカスタマイズ、XDG_CONFIG_HOME の優先度
//!   → prompt テストは daemon 経由フローで検証する
//! - #51–58: `config` コマンド（エディタ起動）

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

const MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const CHECKS_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS"}]"#;
const CONFLICT_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":"APPROVED"}"#;

const BIN: &str = "merge-ready";
const PROMPT_BIN: &str = "merge-ready-prompt";

/// 設定を書いて daemon を起動し、キャッシュが温まった後の `prompt` 出力を検証する。
fn assert_prompt_with_config(env: &TestEnv, expected: &str) {
    let _daemon = DaemonHandle::start(env);
    DaemonHandle::wait_for_cache(env, 5000);

    let mut cmd = Command::cargo_bin("merge-ready-prompt").unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(expected.to_owned()))
        .stderr("");
}

// ── #42–46, #48: prompt 出力契約（パラメータ化） ─────────────────────────────

/// #42 設定なし / #43 symbol / #44 label / #45 format / #46 全フィールド / #48 不正 TOML
#[rstest]
#[case::no_config(None, "✓ merge-ready")]
#[case::custom_symbol(Some("[merge_ready]\nsymbol = \"★\""), "★ merge-ready")]
#[case::custom_label(Some("[merge_ready]\nlabel = \"OK!\""), "✓ OK!")]
#[case::custom_format(
    Some("[merge_ready]\nformat = \"[$symbol] $label\""),
    "[✓] merge-ready"
)]
#[case::all_fields_custom(
    Some("[merge_ready]\nsymbol = \"✅\"\nlabel = \"lgtm\"\nformat = \"$label $symbol\""),
    "lgtm ✅"
)]
#[case::invalid_toml(Some("this is not valid toml ][[["), "✓ merge-ready")]
fn test_config_prompt(#[case] config: Option<&str>, #[case] expected: &str) {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    if let Some(cfg) = config {
        env.write_config(cfg);
    }
    assert_prompt_with_config(&env, expected);
}

// ── #47: 一部セクションのみ設定 ──────────────────────────────────────────────

/// #47: 一部セクションのみ設定 → 未設定セクションはデフォルト値にフォールバック
#[test]
fn test_partial_config_other_tokens_use_defaults() {
    let env = TestEnv::new(CONFLICT_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[conflict]\nsymbol = \"✘\"");
    assert_prompt_with_config(&env, "✘ conflict");
}

// ── #49–50: XDG_CONFIG_HOME ───────────────────────────────────────────────────

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

    // start_with_env はソケット出現を確認してから返るため、
    // prompt が spawn_daemon() を呼んで XDG なし daemon を起動する競合が発生しない。
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[("XDG_CONFIG_HOME", xdg_dir.path().to_str().unwrap())],
    );

    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout("★ merge-ready").stderr("");
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
    cmd.assert().success().stdout("★ merge-ready").stderr("");
}

// ── #51–58: config ────────────────────────────────────────────────────────────

/// #51: `$VISUAL` が設定されている場合、`$VISUAL` がファイルパスを引数として呼ばれる
#[test]
fn test_config_edit_uses_visual() {
    let env = TestEnv::without_gh();
    env.write_config("[merge_ready]\nsymbol = \"★\"");
    let (editor_path, log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut c);
    c.env("VISUAL", &editor_path);
    c.env_remove("EDITOR");
    c.args(["config"]);
    c.assert().success().stderr("");

    let called_path = std::fs::read_to_string(&log_path).expect("editor was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );
}

/// #52: `$VISUAL` 未設定・`$EDITOR` 設定済み → `$EDITOR` が呼ばれる
#[test]
fn test_config_edit_uses_editor_when_visual_unset() {
    let env = TestEnv::without_gh();
    env.write_config("[merge_ready]\nsymbol = \"★\"");
    let (editor_path, log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut c);
    c.env_remove("VISUAL");
    c.env("EDITOR", &editor_path);
    c.args(["config"]);
    c.assert().success().stderr("");

    let called_path = std::fs::read_to_string(&log_path).expect("editor was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );
}

/// #53: `$VISUAL` / `$EDITOR` 未設定 → `vi` にフォールバック
#[test]
fn test_config_edit_falls_back_to_vi() {
    let env = TestEnv::without_gh();
    env.write_config("[merge_ready]\nsymbol = \"★\"");
    let log_path = env.setup_fake_vi();

    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut c);
    c.env_remove("VISUAL");
    c.env_remove("EDITOR");
    c.args(["config"]);
    c.assert().success().stderr("");

    let called_path = std::fs::read_to_string(&log_path).expect("vi was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );
}

/// #54: 設定ファイル不在 → デフォルト設定ファイルを作成してエディタを開く
#[test]
fn test_config_edit_creates_default_when_absent() {
    let env = TestEnv::without_gh();
    let (editor_path, log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut c);
    c.env("VISUAL", &editor_path);
    c.args(["config"]);
    c.assert().success().stderr("");

    let called_path = std::fs::read_to_string(&log_path).expect("editor was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );

    let config_path = env.home_dir.path().join(".config").join("merge-ready.toml");
    assert!(config_path.exists(), "config file was not created");
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(!content.is_empty(), "config file is empty");
}

/// #55: 設定ディレクトリも不在 → ディレクトリと設定ファイルを作成してエディタを開く
#[test]
fn test_config_edit_creates_dir_and_file_when_both_absent() {
    let env = TestEnv::without_gh();
    let (editor_path, log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin(BIN).unwrap();
    c.env("PATH", env.path_env());
    c.env("HOME", env.home());
    c.env("TMPDIR", env.home());
    let xdg_dir = env.home_dir.path().join("no_such_dir");
    c.env("XDG_CONFIG_HOME", &xdg_dir);
    c.current_dir(env.repo_dir.path());
    c.env("VISUAL", &editor_path);
    c.args(["config"]);
    c.assert().success().stderr("");

    let called_path = std::fs::read_to_string(&log_path).expect("editor was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );

    assert!(xdg_dir.exists(), ".config dir was not created");
    assert!(
        xdg_dir.join("merge-ready.toml").exists(),
        "config file was not created"
    );
}

/// #56: エディタが exit 非 0 → merge-ready も exit 非 0
#[test]
fn test_config_edit_exits_nonzero_when_editor_fails() {
    let env = TestEnv::without_gh();
    env.write_config("[merge_ready]\nsymbol = \"★\"");
    let editor_path = env.setup_failing_editor();

    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut c);
    c.env("VISUAL", &editor_path);
    c.args(["config"]);
    c.assert()
        .failure()
        .stderr(predicates::str::contains("failed to edit config"));
}

/// #57: `HOME` / `XDG_CONFIG_HOME` 未設定 → exit 非 0
#[test]
fn test_config_edit_exits_nonzero_without_config_path() {
    let env = TestEnv::without_gh();
    let (editor_path, _log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin(BIN).unwrap();
    c.env("PATH", env.path_env());
    c.env_remove("HOME");
    c.env_remove("XDG_CONFIG_HOME");
    c.current_dir(env.repo_dir.path());
    c.env("VISUAL", &editor_path);
    c.args(["config"]);
    c.assert()
        .failure()
        .stderr(predicates::str::contains("failed to edit config"));
}

/// #58: デフォルト生成内容に各セクションが含まれる
#[test]
fn test_config_edit_default_contains_sections() {
    let env = TestEnv::without_gh();
    let (editor_path, _log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut c);
    c.env("VISUAL", &editor_path);
    c.args(["config"]);
    c.assert().success().stderr("");

    let config_path = env.home_dir.path().join(".config").join("merge-ready.toml");
    let content = std::fs::read_to_string(&config_path).expect("read config");
    assert!(
        content.contains("merge_ready"),
        "config should contain merge_ready section, got:\n{content}"
    );
    assert!(
        content.contains("conflict"),
        "config should contain conflict section, got:\n{content}"
    );
}
