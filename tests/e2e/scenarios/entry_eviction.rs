//! キャッシュエントリ自動削除シナリオ
//!
//! `MERGE_READY_ENTRY_MAX_AGE_SECS=0` により最終 Query からすぐにエントリが期限切れとなる。
//! スケジューラ tick で `retain` が動き、次のクエリでエントリが存在しないため
//! `? loading` に戻ることを確認する。
//!
//! カバー: `CacheEntry::is_expired`, `background_refresh::collect_targets` の eviction ループ

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// エントリ期限切れ → 次クエリで再び `? loading` が返される
#[test]
fn test_entry_expires_and_shows_loading_again() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_ENTRY_MAX_AGE_SECS", "0"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
            ("MERGE_READY_STALE_TTL", "0"),
        ],
    );

    // 初回クエリ: ミス → ? loading
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // キャッシュ温まるまで待つ
    DaemonHandle::wait_for_cache(&env, 5000);

    // スケジューラが entry_max_age_secs=0 で evict するまで待機（1.5 tick 分）
    std::thread::sleep(std::time::Duration::from_millis(1500));

    // エントリが削除されているので再び ? loading が返る
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));
}
