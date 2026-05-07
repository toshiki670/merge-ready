use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

const MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const CHECKS_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS"}]"#;
const CONFLICT_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":"APPROVED"}"#;

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

/// #42 設定なし / #43 symbol / #44 label / #45 format / #46 全フィールド / #48 不正 TOML
#[rstest]
#[case::no_config(None, "✓ Ready for merge")]
#[case::custom_symbol(Some("[merge_ready]\nsymbol = \"★\""), "★ Ready for merge")]
#[case::custom_label(Some("[merge_ready]\nlabel = \"OK!\""), "✓ OK!")]
#[case::custom_format(
    Some("[merge_ready]\nformat = \"[$symbol] $label\""),
    "[✓] Ready for merge"
)]
#[case::all_fields_custom(
    Some("[merge_ready]\nsymbol = \"✅\"\nlabel = \"lgtm\"\nformat = \"$label $symbol\""),
    "lgtm ✅"
)]
#[case::invalid_toml(Some("this is not valid toml ][[["), "✓ Ready for merge")]
fn test_config_prompt(#[case] config: Option<&str>, #[case] expected: &str) {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    if let Some(cfg) = config {
        env.write_config(cfg);
    }
    assert_prompt_with_config(&env, expected);
}

/// #47: 一部セクションのみ設定 → 未設定セクションはデフォルト値にフォールバック
#[test]
fn test_partial_config_other_tokens_use_defaults() {
    let env = TestEnv::new(CONFLICT_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[conflict]\nsymbol = \"✘\"");
    assert_prompt_with_config(&env, "✘ Resolve conflict");
}
