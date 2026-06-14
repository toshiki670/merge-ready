//! `MERGE_READY_LOG_LEVEL` の E2E（#413）。
//!
//! daemon が出す warn/info ログが、設定したレベルに応じて
//! `$HOME/.cache/merge-ready/error.log` に記録されることを検証する。
//! 以前は logger が `LevelFilter::Error` 固定で、warn 以下が全て捨てられていた。

use std::path::Path;
use std::time::{Duration, Instant};

use super::super::helpers::DaemonHandle;
use super::log_level_fixtures;

const POLL_MS: u64 = 5000;

fn error_log_path(home: &Path) -> std::path::PathBuf {
    home.join(".cache/merge-ready/error.log")
}

fn read_log(home: &Path) -> String {
    std::fs::read_to_string(error_log_path(home)).unwrap_or_default()
}

fn rate_limit_call_count(path: &Path) -> usize {
    std::fs::read_to_string(path).unwrap_or_default().len()
}

/// `cond` が真になるまで最大 `max_ms` ミリ秒ポーリングする。
fn wait_until(max_ms: u64, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_millis(max_ms);
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// 行頭が `YYYY-MM-DDT...` 形式（日付を含む RFC3339）かどうかを判定する。
/// 旧実装の時刻のみ書式（例: `09:46:44`）は日付が無いため false になる。
fn starts_with_rfc3339_date(line: &str) -> bool {
    let b = line.as_bytes();
    b.len() >= 11
        && b[0..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'T'
}

/// デフォルト（warn）では `rate_limit` 取得失敗の warn ログが `error.log` に残る。
#[test]
fn warn_log_recorded_at_default_level() {
    let fx = log_level_fixtures::with_failing_rate_limit();
    let _daemon = DaemonHandle::start(&fx.env);
    DaemonHandle::wait_for_cache(&fx.env, 5000);

    let home = fx.env.home().to_path_buf();
    let found = wait_until(POLL_MS, || {
        read_log(&home).contains("rate_limit fetch failed")
    });
    assert!(
        found,
        "default(warn) で rate_limit 失敗の warn が error.log に記録されていない: {:?}",
        read_log(&home)
    );
}

/// `MERGE_READY_LOG_LEVEL=error` では warn ログは捨てられ、`error.log` に残らない。
#[test]
fn warn_log_suppressed_at_error_level() {
    let fx = log_level_fixtures::with_failing_rate_limit();
    let _daemon = DaemonHandle::start_with_env(&fx.env, &[("MERGE_READY_LOG_LEVEL", "error")]);
    DaemonHandle::wait_for_cache(&fx.env, 5000);

    // warn を出す経路（rate_limit fetch）を実際に通ったことを保証する。
    let fired = wait_until(POLL_MS, || rate_limit_call_count(&fx.rate_limit_log) >= 1);
    assert!(fired, "rate_limit fetch が発火しなかった");

    let home = fx.env.home();
    assert!(
        !read_log(home).contains("rate_limit fetch failed"),
        "error レベルなのに warn が記録された: {:?}",
        read_log(home)
    );
}

/// `error.log` の各行は日付を含む RFC3339 タイムスタンプで始まる（#424）。
/// 旧実装は時刻のみ（`HH:MM:SS`）で日付が無く、複数日にまたがると
/// いつ障害が起きたのか特定できなかった。
#[test]
fn error_log_line_starts_with_date() {
    let fx = log_level_fixtures::with_failing_rate_limit();
    let _daemon = DaemonHandle::start(&fx.env);
    DaemonHandle::wait_for_cache(&fx.env, 5000);

    let home = fx.env.home().to_path_buf();
    let found = wait_until(POLL_MS, || {
        read_log(&home).contains("rate_limit fetch failed")
    });
    assert!(found, "warn ログが記録されていない: {:?}", read_log(&home));

    let log = read_log(&home);
    let line = log
        .lines()
        .find(|l| l.contains("rate_limit fetch failed"))
        .expect("warn 行が見つからない");
    assert!(
        starts_with_rfc3339_date(line),
        "行が日付を含む RFC3339 タイムスタンプで始まっていない: {line:?}"
    );
}
