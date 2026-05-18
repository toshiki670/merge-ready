use std::fs::OpenOptions;
use std::path::PathBuf;

use simplelog::{Config, LevelFilter, WriteLogger};

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
/// `$HOME/.cache/merge-ready/error.log` への追記ロガーを初期化する。
/// 失敗は静かに無視する（ログが書けなくてもデーモンは止まらない）。
pub fn init() {
    let Some(path) = log_path() else { return };
    let Ok(file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = WriteLogger::init(LevelFilter::Error, Config::default(), file);
}

fn log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let dir = std::path::Path::new(&home).join(".cache/merge-ready");
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
