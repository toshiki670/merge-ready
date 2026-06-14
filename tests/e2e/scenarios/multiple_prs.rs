//! 複数 PR シナリオ: 1ブランチに複数のオープン PR が存在する場合の表示検証（#256, #355）
//!
//! 複数 PR では同一ステータス（種別）を 1 トークンに集約し、配下の PR 番号を昇順で
//! `#<number>` 付きに列挙する。グループはステータスの初出（= 最小 ID）順に並ぶ。
//! 単一 PR 時は従来どおり `#<number>` を省略する。

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

/// 2つの PR がある場合: 番号昇順で状態を並べて表示する
///
/// PR #200: merge ready (CLEAN, approved)
/// PR #201: draft
/// 期待出力: "✓ Ready for merge #200 ✎ Ready for review #201"
#[test]
fn test_multiple_prs_shown_in_order() {
    let pr_list_json = r#"[
        {"number":200,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":201,"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""}
    ]"#;
    let env = TestEnv::with_pr_list(pr_list_json, Some(r"[]"));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(
            "✓ Ready for merge #200 ✎ Ready for review #201",
        ))
        .stderr("");
}

/// 複数 PR でブロック状態が混在する場合: 各 PR の状態を独立して表示する
///
/// PR #300: merge ready (CLEAN, CI pass)
/// PR #301: レビュー待ち (`BLOCKED`, `REVIEW_REQUIRED`)
/// 期待出力: "✓ Ready for merge #300 @ Assign reviewer #301"
#[test]
fn test_multiple_prs_with_mixed_states() {
    let pr_list_json = r#"[
        {"number":300,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":301,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"REVIEW_REQUIRED","baseRefName":"","headRefName":""}
    ]"#;
    let env = TestEnv::with_pr_list(pr_list_json, Some(r"[]"));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(
            "✓ Ready for merge #300 @ Assign reviewer #301",
        ))
        .stderr("");
}

/// 同一ステータスの複数 PR: 1 トークンに集約し PR 番号を昇順で列挙する（#355）
///
/// PR #100, #101, #102: いずれも draft
/// 期待出力: "✎ Ready for review #100 #101 #102"
#[test]
fn test_same_status_prs_aggregated_into_single_token() {
    let pr_list_json = r#"[
        {"number":100,"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":101,"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":102,"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""}
    ]"#;
    let env = TestEnv::with_pr_list(pr_list_json, Some(r"[]"));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("✎ Ready for review #100 #101 #102"))
        .stderr("");
}

/// 複数ステータス群の集約: 同一ステータスごとに集約し、グループは初出（最小 ID）順に並ぶ（#355）
///
/// PR #100, #101: draft / PR #102, #103: レビュー待ち（`BLOCKED`, `REVIEW_REQUIRED`）
/// 期待出力: "✎ Ready for review #100 #101 @ Assign reviewer #102 #103"
#[test]
fn test_distinct_status_groups_each_aggregated() {
    let pr_list_json = r#"[
        {"number":100,"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":101,"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":102,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"REVIEW_REQUIRED","baseRefName":"","headRefName":""},
        {"number":103,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"REVIEW_REQUIRED","baseRefName":"","headRefName":""}
    ]"#;
    let env = TestEnv::with_pr_list(pr_list_json, Some(r"[]"));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(
            "✎ Ready for review #100 #101 @ Assign reviewer #102 #103",
        ))
        .stderr("");
}

/// Receives and displays daemon responses that exceed the single-read buffer (#406 regression).
///
/// The prompt used to perform a single 512-byte `read()`, so responses longer
/// than that were truncated before JSON decoding and appeared as `? loading`
/// even while the daemon was healthy.
#[test]
fn test_long_response_exceeding_single_read_buffer_is_received_in_full() {
    // Same-status PRs are aggregated into one token with all PR numbers, making
    // the response line much larger than the former 512-byte read buffer.
    let numbers = 1000..1110;
    let pr_list_json = format!(
        "[{}]",
        numbers
            .clone()
            .map(|n| format!(
                r#"{{"number":{n},"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""}}"#
            ))
            .collect::<Vec<_>>()
            .join(",")
    );
    let expected = numbers.fold(String::from("✎ Ready for review"), |mut s, n| {
        use std::fmt::Write as _;
        let _ = write!(s, " #{n}");
        s
    });
    assert!(
        expected.len() > 512,
        "fixture must exceed the 512B single-read buffer (was {} bytes)",
        expected.len()
    );

    let env = TestEnv::with_pr_list(&pr_list_json, Some(r"[]"));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(expected))
        .stderr("");
}
