use std::fs::OpenOptions;
use std::path::PathBuf;

use simplelog::{Config, LevelFilter, WriteLogger};

use crate::contexts::prompt::application::errors::{ErrorCategory, LogRecord};

pub struct Logger;

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

impl crate::contexts::prompt::application::errors::ErrorLogger for Logger {
    fn log(&self, record: &LogRecord) {
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
}
