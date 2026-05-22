//! スタイル構文（`[text](style)`）の E2E テスト

const PROMPT_BIN: &str = "merge-ready-prompt";
const MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const CHECKS_PASS_JSON: &str =
    r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]"#;

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

/// スタイル構文付き format を設定した場合、ANSI エスケープコードが出力に含まれる。
#[test]
fn styled_format_produces_ansi_in_output() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nformat = \"[$symbol](bold green) $label\"");

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\x1b["))
        .stderr("");
}

/// スタイル構文なしの format（デフォルト）では ANSI コードを出力しない（後方互換）。
#[test]
fn plain_format_produces_no_ansi() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("✓ Ready for merge"))
        .stderr("");
}

/// `[text](style)` の content 内に conditional ブロックを置いた場合の検証。
///
/// `[$symbol $label( $pr_ids)](cyan)` — 単独 PR（`$pr_ids` が空）のとき
/// `( $pr_ids)` ブロックが非表示になり、全体に cyan スタイルが適用される。
#[test]
fn conditional_inside_styled_block_hidden_when_pr_ids_empty() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nformat = \"[$symbol $label( $pr_ids)](cyan)\"");

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    // ANSI コードが含まれる（cyan スタイル適用）
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\x1b["))
        .stderr("");

    // Conditional が非表示になり、`(` も `$pr_ids` リテラルも残らないこと
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.timeout(std::time::Duration::from_secs(5))
        .assert()
        .success()
        .stdout(predicate::str::contains("(").not())
        .stdout(predicate::str::contains("$pr_ids").not())
        .stdout(predicate::str::contains("✓ Ready for merge"));
}

/// 複数の色指定形式（Ansi256 / RGB / Named fg+bg）を使った format で、
/// スタイル付き出力が有効になることを検証する E2E テスト。
#[rstest]
#[case::ansi256_fg("[$symbol](fg:196) $label")]
#[case::rgb_fg("[$symbol](fg:#ff0000) $label")]
#[case::named_fg_bg("[$symbol](red bg:blue) $label")]
#[case::ansi256_fg_rgb_bg("[$symbol](fg:196 bg:#001122) $label")]
/// サポートされる named color（通常色 / bright 色）指定で
/// ANSI エスケープコード付き出力になることを確認する。
#[case::black("[$symbol](black) $label")]
#[case::yellow("[$symbol](yellow) $label")]
#[case::purple("[$symbol](purple) $label")]
#[case::white("[$symbol](white) $label")]
#[case::bright_black("[$symbol](bright-black) $label")]
#[case::bright_red("[$symbol](bright-red) $label")]
#[case::bright_green("[$symbol](bright-green) $label")]
#[case::bright_yellow("[$symbol](bright-yellow) $label")]
#[case::bright_blue("[$symbol](bright-blue) $label")]
#[case::bright_purple("[$symbol](bright-purple) $label")]
#[case::bright_cyan("[$symbol](bright-cyan) $label")]
#[case::bright_white("[$symbol](bright-white) $label")]
fn multi_color_format_produces_ansi_output(#[case] fmt: &str) {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config(&format!("[merge_ready]\nformat = \"{fmt}\""));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\x1b["))
        .stderr("");
}

/// スタイル適用後のテキストに色が漏れない（reset が挿入される）。
/// `[$symbol](bold green) $label` の $label 部分はデフォルトカラーで出力される。
#[test]
fn text_after_styled_segment_is_not_colored() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nformat = \"[$symbol](bold green) $label\"");

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    // $label ("Ready for merge") がプレーンテキストとして末尾にある。
    cmd.assert()
        .success()
        .stdout(predicate::str::ends_with("Ready for merge"))
        .stderr("");
}
