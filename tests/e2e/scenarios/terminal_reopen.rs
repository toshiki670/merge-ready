//! Terminal PR 再オープン E2E テスト
//!
//! #H: MERGED 状態の PR が再オープンされたとき、`reset_to_warm` でエントリが Warm に戻り
//!     gh が再フェッチされて open 状態の出力に切り替わること

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::DaemonHandle;
use super::terminal_reopen_fixtures;

/// #H: Terminal (MERGED) → TTL 超過 → クエリで `reset_to_warm` → gh が OPEN PR を返す
#[test]
fn test_terminal_pr_reopened_resets_to_warm() {
    let env = terminal_reopen_fixtures::with_reopened_pr();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            // Terminal エントリの effective_ttl = warm_refresh_secs
            // stale_ttl=0 にして reset_to_warm 後の OPEN PR フェッチが完了すれば即座に出力が得られるようにする
            ("MERGE_READY_WARM_REFRESH_SECS", "1"),
            ("MERGE_READY_STALE_TTL", "0"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    // 初回: MERGED PR → output="" (terminal)
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::is_empty());

    // warm_refresh_secs=1 → Terminal エントリは is_fresh(1) = elapsed.as_secs() <= 1 → elapsed >= 2s で stale
    std::thread::sleep(std::time::Duration::from_secs(2));

    // クエリ → is_terminal=true かつ NeedsRefresh → reset_to_warm → bg refresh (OPEN PR) 開始
    // この時点では Refreshing 状態で古い "" が返る
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success(); // 空または "? loading" のどちらでも可

    // bg refresh (OPEN PR) が完了するまで待つ。fake gh は即時応答するので 1 秒で十分。
    std::thread::sleep(std::time::Duration::from_secs(1));

    // OPEN PR の状態が出力されること（非空）
    // fetched_at が更新されたので stale_ttl=2 以内なら is_fresh=true で返る
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}
