//! Terminal PR 再オープンシナリオ
//!
//! MERGED → Terminal キャッシュ → stale → `reset_to_warm()` → 再フェッチ → OPEN PR 表示。
//!
//! カバー:
//! - `CacheEntry::reset_to_warm`
//! - `request_handler` の Terminal stale パス
//! - `RefreshPolicy::effective_ttl` の Terminal 分岐

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::cargo::cargo_bin;

use super::super::helpers::{DaemonHandle, run_prompt_with_timeout};
use super::terminal_pr_reset_fixtures;

/// MERGED PR がキャッシュされた後に re-open されると出力が再表示される
#[test]
fn test_terminal_pr_reset_to_warm_after_reopen() {
    let env = terminal_pr_reset_fixtures::with_merged_then_open_pr();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_STALE_TTL", "0"),
            ("MERGE_READY_WARM_REFRESH_SECS", "0"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    // 初回クエリ: ミス → ? loading
    let bin = cargo_bin(PROMPT_BIN);
    let out = run_prompt_with_timeout(
        std::process::Command::new(&bin)
            .env("PATH", env.path_env())
            .env("HOME", env.home())
            .env("TMPDIR", env.home())
            .current_dir(env.repo.path()),
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "? loading");

    // キャッシュ確定（MERGED → Terminal → 空出力）を待つ
    DaemonHandle::wait_for_cache(&env, 5000);

    // stale Terminal エントリへのクエリ → reset_to_warm() 発火 → 再フェッチ予約
    run_prompt_with_timeout(
        std::process::Command::new(&bin)
            .env("PATH", env.path_env())
            .env("HOME", env.home())
            .env("TMPDIR", env.home())
            .current_dir(env.repo.path()),
    );

    // 再フェッチ完了（OPEN PR）で出力が非空になるまでポーリング
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(5000);
    loop {
        let out = run_prompt_with_timeout(
            std::process::Command::new(&bin)
                .env("PATH", env.path_env())
                .env("HOME", env.home())
                .env("TMPDIR", env.home())
                .current_dir(env.repo.path()),
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        let s = stdout.trim();
        if !s.is_empty() && s != "? loading" {
            // OPEN PR → 何らかの PR 出力が表示されるはず
            assert!(
                out.status.success(),
                "merge-ready-prompt should succeed after re-open"
            );
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "re-opened PR output did not appear within 5000ms"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
