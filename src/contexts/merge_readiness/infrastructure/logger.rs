use std::fs::{self, OpenOptions};
use std::io::Write as _;

use crate::contexts::merge_readiness::application::errors::ErrorLogger;

pub struct Logger;

/// `$HOME/.cache/ci-status/error.log` にエラーメッセージを追記する。
///
/// ディレクトリが存在しない場合は自動的に作成する。
/// 書き込み失敗は静かに握り潰す（`stderr` には何も出力しない）。
pub fn append_error(message: &str) {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let log_dir = std::path::Path::new(&home).join(".cache/ci-status");
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("error.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return;
    };
    let _ = writeln!(file, "{message}");
}

impl ErrorLogger for Logger {
    fn log(&self, msg: &str) {
        append_error(msg);
    }
}
