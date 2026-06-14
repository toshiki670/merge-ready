use std::fs::OpenOptions;
use std::path::PathBuf;

use simplelog::{ConfigBuilder, LevelFilter, WriteLogger};

use crate::contexts::evaluation::domain::error::RepositoryError;

/// ロギングのためのエラーカテゴリ（横断的関心事）。
pub enum ErrorCategory {
    Auth,
    RateLimit,
    Timeout,
    Unknown,
}

/// ログに記録する構造化エントリ。
pub struct LogRecord {
    pub category: ErrorCategory,
    pub detail: Option<String>,
}

/// デーモン起動時に一度だけ呼ぶ。
/// `<cache>/merge-ready/error.log` への追記ロガーを初期化する。
/// 失敗は静かに無視する（ログが書けなくてもデーモンは止まらない）。
pub fn init() {
    let Some(path) = log_path() else { return };
    let Ok(file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    // simplelog の既定書式は時刻のみ（`HH:MM:SS`）で日付を含まないため、
    // 障害発生日を特定できるよう日付込みの RFC3339（UTC, 例 `2026-06-14T09:46:44Z`）にする。
    let config = ConfigBuilder::new().set_time_format_rfc3339().build();
    let _ = WriteLogger::init(log_level(), config, file);
}

/// `MERGE_READY_LOG_LEVEL` からロガーの記録レベルを決める。
/// 既定は `warn`（backoff 突入・`rate_limit` 取得失敗・daemon 自己終了といった
/// 運用上重要なイベントを残すため）。
fn log_level() -> LevelFilter {
    parse_log_level(std::env::var("MERGE_READY_LOG_LEVEL").ok().as_deref())
}

/// `off`/`error`/`warn`/`info`/`debug`/`trace`（大文字小文字無視）を解釈する。
/// 未設定・空・不明値は `warn`。
fn parse_log_level(value: Option<&str>) -> LevelFilter {
    match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("off") => LevelFilter::Off,
        Some("error") => LevelFilter::Error,
        Some("info") => LevelFilter::Info,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Warn,
    }
}

// XDG_CACHE_HOME が有効な絶対パスならそちらを優先し、無効（未設定・空・相対パス）なら
// $HOME/.cache にフォールバックする。設定 (XDG 対応) とログの解決方針を揃える。
fn log_path() -> Option<PathBuf> {
    let base = match super::xdg::base_dir("XDG_CACHE_HOME") {
        Some(dir) => dir,
        None => PathBuf::from(std::env::var_os("HOME")?).join(".cache"),
    };
    let dir = base.join("merge-ready");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("error.log"))
}

/// 構造化ログを 1 行書き出す。
pub fn log_record(record: &LogRecord) {
    let category = match record.category {
        ErrorCategory::Auth => "Auth",
        ErrorCategory::RateLimit => "RateLimit",
        ErrorCategory::Timeout => "Timeout",
        ErrorCategory::Unknown => "Unknown",
    };
    match &record.detail {
        Some(detail) => log::error!("[{category}] {detail}"),
        None => log::error!("[{category}]"),
    }
}

/// Shell から呼び出される、`RepositoryError` 用のロギングエントリポイント。
/// 旧 `into_token` の `RateLimited` 分岐で書いていたログをここで集約する。
pub fn log_repository_error(e: RepositoryError) {
    if let RepositoryError::RateLimited = e {
        log_record(&LogRecord {
            category: ErrorCategory::RateLimit,
            detail: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_level_unset_defaults_to_warn() {
        assert_eq!(parse_log_level(None), LevelFilter::Warn);
    }

    #[test]
    fn parse_log_level_unknown_defaults_to_warn() {
        assert_eq!(parse_log_level(Some("verbose")), LevelFilter::Warn);
        assert_eq!(parse_log_level(Some("")), LevelFilter::Warn);
    }

    #[test]
    fn parse_log_level_recognises_each_level() {
        assert_eq!(parse_log_level(Some("off")), LevelFilter::Off);
        assert_eq!(parse_log_level(Some("error")), LevelFilter::Error);
        assert_eq!(parse_log_level(Some("warn")), LevelFilter::Warn);
        assert_eq!(parse_log_level(Some("info")), LevelFilter::Info);
        assert_eq!(parse_log_level(Some("debug")), LevelFilter::Debug);
        assert_eq!(parse_log_level(Some("trace")), LevelFilter::Trace);
    }

    #[test]
    fn parse_log_level_is_case_and_whitespace_insensitive() {
        assert_eq!(parse_log_level(Some("  INFO ")), LevelFilter::Info);
        assert_eq!(parse_log_level(Some("Debug")), LevelFilter::Debug);
    }
}
