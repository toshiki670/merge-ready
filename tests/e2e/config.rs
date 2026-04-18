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
