//! エントリ有効期限 E2E テスト
//!
//! #C: `entry_max_age_secs` を超えてクエリが来なかったエントリは削除される。
//!     次のクエリで "? loading" が返り、エントリが再作成される。

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// #C: クエリが途絶えた後に `entry_max_age_secs` が経過するとエントリが削除される
#[test]
fn test_entry_evicted_after_max_age_without_query() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_ENTRY_MAX_AGE_SECS", "2"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    // 初回: キャッシュミス → "? loading"
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // キャッシュが温まるまで待つ（ここでのクエリが last_queried_at を更新する）
    DaemonHandle::wait_for_cache(&env, 5000);

    // 3 秒間クエリを送らない → entry_max_age_secs=2 を超えてエントリが削除される
    std::thread::sleep(std::time::Duration::from_secs(3));

    // 再クエリ: エントリが削除されているので "? loading" に戻る
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));
}
