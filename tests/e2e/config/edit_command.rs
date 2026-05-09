//! `merge-ready config` サブコマンドの E2E テスト（シナリオ #51–58）
//!
//! - #51–53: エディタ選択（$VISUAL → $EDITOR → vi フォールバック）
//! - #54–55: 設定ファイル / ディレクトリ不在時の自動作成
//! - #56–57: エラーケース（エディタ失敗、HOME/XDG_CONFIG_HOME 未設定）
//! - #58: デフォルト生成内容の検証

use assert_cmd::Command;

use super::super::helpers::TestEnv;

const BIN: &str = "merge-ready";

// ── #51–53: エディタ選択 ──────────────────────────────────────────────────────

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

// ── #54–55: 設定ファイル / ディレクトリの自動作成 ────────────────────────────

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

// ── #56–57: エラーケース ──────────────────────────────────────────────────────

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

// ── #58: デフォルト生成内容の検証 ────────────────────────────────────────────

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
