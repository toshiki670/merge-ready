use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::contexts::prompt::application::errors::{ErrorCategory, LogRecord};

pub struct Logger;

pub fn append_log_record(record: &LogRecord) {
    let category = match record.category {
        ErrorCategory::Auth => "Auth",
        ErrorCategory::RateLimit => "RateLimit",
        ErrorCategory::Timeout => "Timeout",
        ErrorCategory::Unknown => "Unknown",
    };
    let message = match &record.detail {
        Some(d) => format!("[{category}] {d}"),
        None => format!("[{category}]"),
    };
    append_line(&message);
}

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // ISO 8601 相当の簡易フォーマット（外部クレートなし）
    let (y, mo, d, h, mi, s) = secs_to_datetime(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn append_line(message: &str) {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let log_dir = std::path::Path::new(&home).join(".cache/merge-ready");
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("error.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return;
    };
    let _ = writeln!(file, "{} {message}", timestamp());
}

/// Unix タイムスタンプ（秒）を (year, month, day, hour, min, sec) に変換する。
fn secs_to_datetime(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec = secs % 60;
    let total_min = secs / 60;
    let min = total_min % 60;
    let total_hours = total_min / 60;
    let hour = total_hours % 24;
    let total_days = total_hours / 24;

    // グレゴリオ暦への変換（ユリウス通日ベース）
    let jdn = total_days + 719_468;
    let era = jdn / 146_097;
    let doe = jdn - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    (year, month, day, hour, min, sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_epoch() {
        assert_eq!(secs_to_datetime(0), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn timestamp_known() {
        // 2026-04-24T00:00:00Z = 1776988800
        assert_eq!(secs_to_datetime(1_776_988_800), (2026, 4, 24, 0, 0, 0));
    }
}
