//! 設定ファイル（`~/.config/merge-ready.toml`）の E2E テスト
//!
//! symbol / label / format のカスタマイズ、部分設定のフォールバック、
//! 設定ファイルなし・不正 TOML でのデフォルト出力を検証する。

use super::helpers::TestEnv;
use assert_cmd::Command;

const MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const CHECKS_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS"}]"#;
const CONFLICT_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":"APPROVED"}"#;

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.args(["prompt", "--no-cache"]);
    c
}

/// 設定ファイルなし → デフォルトのシンボル・ラベルで出力
#[test]
fn test_no_config_uses_defaults() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    cmd(&env)
        .assert()
        .success()
        .stdout("✓ merge-ready")
        .stderr("");
}

/// `symbol` のみカスタマイズ → カスタムシンボル + デフォルトラベル
#[test]
fn test_custom_symbol() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nsymbol = \"★\"");
    cmd(&env)
        .assert()
        .success()
        .stdout("★ merge-ready")
        .stderr("");
}

/// `label` のみカスタマイズ → デフォルトシンボル + カスタムラベル
#[test]
fn test_custom_label() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nlabel = \"OK!\"");
    cmd(&env).assert().success().stdout("✓ OK!").stderr("");
}

/// `format` をカスタマイズ → 順序・区切りが変わる
#[test]
fn test_custom_format() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nformat = \"[$symbol] $label\"");
    cmd(&env)
        .assert()
        .success()
        .stdout("[✓] merge-ready")
        .stderr("");
}

/// `symbol` / `label` / `format` を全部カスタマイズ
#[test]
fn test_all_fields_custom() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config(
        "[merge_ready]\nsymbol = \"✅\"\nlabel = \"lgtm\"\nformat = \"$label $symbol\"",
    );
    cmd(&env).assert().success().stdout("lgtm ✅").stderr("");
}

/// 一部セクションのみ設定 → 未設定セクションはデフォルト値にフォールバック
#[test]
fn test_partial_config_other_tokens_use_defaults() {
    let env = TestEnv::new(CONFLICT_JSON, Some(CHECKS_PASS_JSON));
    // conflict のみカスタマイズ、他はデフォルト
    env.write_config("[conflict]\nsymbol = \"✘\"");
    cmd(&env).assert().success().stdout("✘ conflict").stderr("");
}

/// 不正な TOML → デフォルト出力にフォールバック（パニックしない）
#[test]
fn test_invalid_toml_falls_back_to_defaults() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("this is not valid toml ][[[");
    cmd(&env)
        .assert()
        .success()
        .stdout("✓ merge-ready")
        .stderr("");
}

/// XDG_CONFIG_HOME が設定されている場合、そちらから設定を読む
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

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.env("XDG_CONFIG_HOME", xdg_dir.path());
    c.args(["prompt", "--no-cache"]);
    c.assert().success().stdout("★ merge-ready").stderr("");
}

/// XDG_CONFIG_HOME と HOME が両方ある場合、XDG_CONFIG_HOME が優先される
#[test]
fn test_xdg_config_home_takes_precedence_over_home() {
    use std::fs;
    use tempfile::tempdir;

    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    // HOME 側にも設定を置くが XDG 側が優先されるはず
    env.write_config("[merge_ready]\nsymbol = \"✓\"");

    let xdg_dir = tempdir().expect("tempdir");
    fs::write(
        xdg_dir.path().join("merge-ready.toml"),
        "[merge_ready]\nsymbol = \"★\"",
    )
    .expect("write xdg config");

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.env("XDG_CONFIG_HOME", xdg_dir.path());
    c.args(["prompt", "--no-cache"]);
    c.assert().success().stdout("★ merge-ready").stderr("");
}

// ============================================================
// config edit
// ============================================================

/// $VISUAL が設定されている場合、$VISUAL がファイルパスを引数として呼ばれる
#[test]
fn test_config_edit_uses_visual() {
    let env = TestEnv::without_gh();
    env.write_config("[merge_ready]\nsymbol = \"★\"");
    let (editor_path, log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.env("VISUAL", &editor_path);
    c.env_remove("EDITOR");
    c.args(["config", "edit"]);
    c.assert().success().stderr("");

    let called_path = std::fs::read_to_string(&log_path).expect("editor was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );
}

/// 設定ファイルあり・$VISUAL 未設定・$EDITOR 設定済み → $EDITOR が呼ばれる
#[test]
fn test_config_edit_uses_editor_when_visual_unset() {
    let env = TestEnv::without_gh();
    env.write_config("[merge_ready]\nsymbol = \"★\"");
    let (editor_path, log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.env_remove("VISUAL");
    c.env("EDITOR", &editor_path);
    c.args(["config", "edit"]);
    c.assert().success().stderr("");

    let called_path = std::fs::read_to_string(&log_path).expect("editor was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );
}

/// 設定ファイルあり・$VISUAL / $EDITOR 未設定 → vi がフォールバックとして呼ばれる
#[test]
fn test_config_edit_falls_back_to_vi() {
    let env = TestEnv::without_gh();
    env.write_config("[merge_ready]\nsymbol = \"★\"");
    let log_path = env.setup_fake_vi();

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.env_remove("VISUAL");
    c.env_remove("EDITOR");
    c.args(["config", "edit"]);
    c.assert().success().stderr("");

    let called_path = std::fs::read_to_string(&log_path).expect("vi was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );
}

/// 設定ファイルなし・エディタ設定済み → デフォルト設定ファイルが作成されてからエディタが呼ばれる
#[test]
fn test_config_edit_creates_default_when_absent() {
    let env = TestEnv::without_gh();
    // 設定ファイルを書かない（.config/ ディレクトリは apply() で XDG として設定されるが、ファイルは存在しない）
    let (editor_path, log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.env("VISUAL", &editor_path);
    c.args(["config", "edit"]);
    c.assert().success().stderr("");

    // エディタが呼ばれたことを確認
    let called_path = std::fs::read_to_string(&log_path).expect("editor was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );

    // デフォルト設定ファイルが作成されていることを確認
    let config_path = env.home_dir.path().join(".config").join("merge-ready.toml");
    assert!(config_path.exists(), "config file was not created");
    let content = std::fs::read_to_string(&config_path).expect("read config");
    assert!(!content.is_empty(), "config file is empty");
}

/// 設定ファイルなし・.config/ ディレクトリも存在しない → ディレクトリ作成してからファイル作成してエディタ呼ぶ
#[test]
fn test_config_edit_creates_dir_and_file_when_both_absent() {
    let env = TestEnv::without_gh();
    // .config/ ディレクトリも作らない。apply() で XDG_CONFIG_HOME を設定するが、そのディレクトリは存在しない。
    // ただし apply() は XDG_CONFIG_HOME を home_dir/.config に設定するので、ここでは別の XDG を使う。
    let (editor_path, log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    // apply() を使わず手動で設定（ディレクトリが存在しない XDG を使う）
    c.env("PATH", env.path_env());
    c.env("HOME", env.home());
    c.env("TMPDIR", env.home());
    // XDG_CONFIG_HOME を存在しないディレクトリに指定
    let xdg_dir = env.home_dir.path().join("no_such_dir");
    c.env("XDG_CONFIG_HOME", &xdg_dir);
    c.current_dir(env.repo_dir.path());
    c.env("VISUAL", &editor_path);
    c.args(["config", "edit"]);
    c.assert().success().stderr("");

    // エディタが呼ばれたことを確認
    let called_path = std::fs::read_to_string(&log_path).expect("editor was not called");
    assert!(
        called_path.ends_with("merge-ready.toml"),
        "expected merge-ready.toml, got: {called_path}"
    );

    // ディレクトリとファイルが作成されていることを確認
    assert!(xdg_dir.exists(), ".config dir was not created");
    assert!(
        xdg_dir.join("merge-ready.toml").exists(),
        "config file was not created"
    );
}

/// エディタが非ゼロ終了した場合、merge-ready も非ゼロ終了し stderr にメッセージを出す
#[test]
fn test_config_edit_exits_nonzero_when_editor_fails() {
    let env = TestEnv::without_gh();
    env.write_config("[merge_ready]\nsymbol = \"★\"");
    let editor_path = env.setup_failing_editor();

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.env("VISUAL", &editor_path);
    c.args(["config", "edit"]);
    c.assert()
        .failure()
        .stderr(predicates::str::contains("failed to edit config"));
}

/// HOME も XDG_CONFIG_HOME も未設定の場合、merge-ready は非ゼロ終了し stderr にメッセージを出す
#[test]
fn test_config_edit_exits_nonzero_without_config_path() {
    let env = TestEnv::without_gh();
    let (editor_path, _log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    c.env("PATH", env.path_env());
    c.env_remove("HOME");
    c.env_remove("XDG_CONFIG_HOME");
    c.current_dir(env.repo_dir.path());
    c.env("VISUAL", &editor_path);
    c.args(["config", "edit"]);
    c.assert()
        .failure()
        .stderr(predicates::str::contains("failed to edit config"));
}

/// 設定ファイルなし → デフォルト設定ファイルがデフォルト値付きセクションを含む
#[test]
fn test_config_edit_default_contains_sections() {
    let env = TestEnv::without_gh();
    let (editor_path, _log_path) = env.setup_fake_editor();

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.env("VISUAL", &editor_path);
    c.args(["config", "edit"]);
    c.assert().success().stderr("");

    let config_path = env.home_dir.path().join(".config").join("merge-ready.toml");
    let content = std::fs::read_to_string(&config_path).expect("read config");
    assert!(
        content.contains("merge_ready"),
        "config should contain merge_ready section with defaults, got:\n{content}"
    );
    assert!(
        content.contains("conflict"),
        "config should contain conflict section with defaults, got:\n{content}"
    );
}

// ============================================================
// config update
// ============================================================

/// 設定ファイルなし → デフォルト設定ファイルが新規作成される
#[test]
fn test_config_update_creates_default_when_absent() {
    let env = TestEnv::without_gh();

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.args(["config", "update"]);
    c.assert().success().stderr("");

    let config_path = env.home_dir.path().join(".config").join("merge-ready.toml");
    assert!(config_path.exists(), "config file was not created");
    let content = std::fs::read_to_string(&config_path).expect("read config");
    assert!(!content.is_empty(), "config file is empty");
}

/// バージョンが最新 → ファイルが変更されない
#[test]
fn test_config_update_no_change_when_latest_version() {
    let env = TestEnv::without_gh();
    let original = "version = 1\n\n[merge_ready]\nsymbol = \"★\"\n";
    env.write_config(original);

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.args(["config", "update"]);
    c.assert().success().stderr("");

    let config_path = env.home_dir.path().join(".config").join("merge-ready.toml");
    let content = std::fs::read_to_string(&config_path).expect("read config");
    assert_eq!(content, original, "file should not be modified");
}

/// 旧バージョン・有効なキーあり → 既存の値が保持され version が更新される
#[test]
fn test_config_update_preserves_valid_keys() {
    let env = TestEnv::without_gh();
    // version なし（旧バージョン）・有効なキーあり
    env.write_config("[merge_ready]\nsymbol = \"★\"\n");

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.args(["config", "update"]);
    c.assert().success().stderr("");

    let config_path = env.home_dir.path().join(".config").join("merge-ready.toml");
    let content = std::fs::read_to_string(&config_path).expect("read config");
    // symbol が保持されている
    assert!(
        content.contains("★"),
        "symbol should be preserved, got:\n{content}"
    );
    // version が最新になっている
    assert!(
        content.contains("version = 1"),
        "version should be updated, got:\n{content}"
    );
}

/// 旧バージョン・廃止キーあり → 廃止キーが削除される
#[test]
fn test_config_update_removes_obsolete_keys() {
    let env = TestEnv::without_gh();
    // 廃止キー（Config struct に存在しないフィールド）を含む設定ファイル
    env.write_config("[merge_ready]\nsymbol = \"★\"\n\n[obsolete_section]\nsome_key = \"value\"\n");

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.args(["config", "update"]);
    c.assert().success().stderr("");

    let config_path = env.home_dir.path().join(".config").join("merge-ready.toml");
    let content = std::fs::read_to_string(&config_path).expect("read config");
    assert!(
        !content.contains("obsolete_section"),
        "obsolete_section should be removed, got:\n{content}"
    );
    assert!(
        !content.contains("some_key"),
        "some_key should be removed, got:\n{content}"
    );
}

/// 旧バージョン・不足キーあり → デフォルト値で不足キーが追加される
#[test]
fn test_config_update_adds_missing_sections_with_defaults() {
    let env = TestEnv::without_gh();
    // [conflict] セクションが存在しない（version なし）
    env.write_config("[merge_ready]\nsymbol = \"★\"\n");

    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.args(["config", "update"]);
    c.assert().success().stderr("");

    let config_path = env.home_dir.path().join(".config").join("merge-ready.toml");
    let content = std::fs::read_to_string(&config_path).expect("read config");
    // 既存の symbol が保持されている
    assert!(
        content.contains("★"),
        "symbol should be preserved, got:\n{content}"
    );
    // 不足していた [conflict] セクションがデフォルト値で追加されている
    assert!(
        content.contains("conflict"),
        "conflict section should be added with defaults, got:\n{content}"
    );
}
