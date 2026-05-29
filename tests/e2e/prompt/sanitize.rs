//! 出力サニタイズの E2E テスト（Issue #374）
//!
//! `merge-ready-prompt` は daemon から受け取った出力を端末へ表示する前に、
//! 危険な制御文字を除去する。正当な ANSI カラー（SGR）は保持し、改行などの
//! 制御文字は除去してプロンプト破壊・複数行注入を防ぐ。

const PROMPT_BIN: &str = "merge-ready-prompt";
const MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const CHECKS_PASS_JSON: &str =
    r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]"#;

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

/// format に埋め込まれた改行（制御文字）はプロンプト出力から除去される。
///
/// daemon 側の render は format 文字列をそのまま展開するため、設定に改行を
/// 含めると出力にも改行が乗る。prompt バイナリのサニタイズがこれを除去する
/// ことで、出力が 1 行に保たれる（複数行注入・プロンプト破壊の防止）。
#[test]
fn newline_in_output_is_stripped() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    // TOML basic string の `\n` は実際の改行文字に展開される。
    env.write_config("[merge_ready]\nformat = \"$symbol\\n$label\"");

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    // 改行が除去され、symbol と label が連結された 1 行になる。
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\n").not())
        .stdout(predicate::str::diff("✓Ready for merge"))
        .stderr("");
}

/// サニタイズ後も正当な ANSI カラー（SGR）は保持される（後方互換）。
#[test]
fn sgr_color_is_preserved_after_sanitize() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nformat = \"[$symbol](bold green) $label\"");

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        // SGR シーケンスは残り、末尾のプレーンテキストも保持される。
        .stdout(predicate::str::contains("\x1b["))
        .stdout(predicate::str::ends_with("Ready for merge"))
        .stderr("");
}
