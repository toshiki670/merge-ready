use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use simplelog::{ConfigBuilder, LevelFilter, WriteLogger};

use crate::contexts::evaluation::domain::error::RepositoryError;

const DEFAULT_LOG_MAX_BYTES: u64 = 1024 * 1024;
const DEFAULT_LOG_MAX_BACKUPS: u32 = 4;
const LOG_MAX_BYTES_ENV: &str = "MERGE_READY_LOG_MAX_BYTES";
const LOG_MAX_BACKUPS_ENV: &str = "MERGE_READY_LOG_MAX_BACKUPS";

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
    let Ok(writer) = RotatingLogWriter::open(&path, LogRotationPolicy::from_env()) else {
        return;
    };
    // simplelog の既定書式は時刻のみ（`HH:MM:SS`）で日付を含まないため、
    // 障害発生日を特定できるよう日付込みの RFC3339（UTC, 例 `2026-06-14T09:46:44Z`）にする。
    let config = ConfigBuilder::new().set_time_format_rfc3339().build();
    let _ = WriteLogger::init(log_level(), config, writer);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LogRotationPolicy {
    max_bytes: u64,
    max_backups: u32,
}

impl LogRotationPolicy {
    #[must_use]
    const fn new(max_bytes: u64, max_backups: u32) -> Self {
        Self {
            max_bytes,
            max_backups,
        }
    }

    fn from_env() -> Self {
        Self::parse(
            std::env::var(LOG_MAX_BYTES_ENV).ok().as_deref(),
            std::env::var(LOG_MAX_BACKUPS_ENV).ok().as_deref(),
        )
    }

    fn parse(max_bytes: Option<&str>, max_backups: Option<&str>) -> Self {
        Self::new(
            parse_positive_u64(max_bytes, DEFAULT_LOG_MAX_BYTES),
            parse_positive_u32(max_backups, DEFAULT_LOG_MAX_BACKUPS),
        )
    }
}

fn parse_positive_u64(value: Option<&str>, default: u64) -> u64 {
    value
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn parse_positive_u32(value: Option<&str>, default: u32) -> u32 {
    value
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

struct RotatingLogWriter {
    path: PathBuf,
    file: Option<File>,
    current_size: u64,
    policy: LogRotationPolicy,
}

impl RotatingLogWriter {
    fn open(path: &Path, policy: LogRotationPolicy) -> io::Result<Self> {
        if std::fs::metadata(path).is_ok_and(|meta| meta.len() > policy.max_bytes) {
            rotate_files(path, policy.max_backups)?;
        }

        let current_size = std::fs::metadata(path).map_or(0, |meta| meta.len());
        Ok(Self {
            path: path.to_path_buf(),
            file: Some(open_active_log(path)?),
            current_size,
            policy,
        })
    }

    fn file_mut(&mut self) -> io::Result<&mut File> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file is not open"))
    }

    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        rotate_files(&self.path, self.policy.max_backups)?;
        self.file = Some(open_active_log(&self.path)?);
        self.current_size = 0;
        Ok(())
    }
}

impl Write for RotatingLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let incoming = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        if self.current_size.saturating_add(incoming) > self.policy.max_bytes {
            self.rotate()?;
        }

        let written = self.file_mut()?.write(buf)?;
        let written_size = u64::try_from(written).unwrap_or(u64::MAX);
        self.current_size = self.current_size.saturating_add(written_size);

        if self.current_size > self.policy.max_bytes {
            self.rotate()?;
        }

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file_mut()?.flush()
    }
}

fn open_active_log(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn rotate_files(path: &Path, max_backups: u32) -> io::Result<()> {
    remove_if_exists(&backup_path(path, max_backups))?;

    for index in (1..max_backups).rev() {
        rename_if_exists(&backup_path(path, index), &backup_path(path, index + 1))?;
    }

    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > 0 => std::fs::rename(path, backup_path(path, 1))?,
        Ok(_) => remove_if_exists(path)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    Ok(())
}

fn rename_if_exists(from: &Path, to: &Path) -> io::Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn backup_path(path: &Path, index: u32) -> PathBuf {
    let Some(file_name) = path.file_name() else {
        return PathBuf::from(format!("{}.{index}", path.display()));
    };
    let mut backup_name = file_name.to_os_string();
    backup_name.push(format!(".{index}"));
    path.with_file_name(backup_name)
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
    use std::io::Write;

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

    #[test]
    fn log_rotation_policy_defaults_invalid_values() {
        assert_eq!(
            LogRotationPolicy::parse(Some("0"), Some("invalid")),
            LogRotationPolicy {
                max_bytes: DEFAULT_LOG_MAX_BYTES,
                max_backups: DEFAULT_LOG_MAX_BACKUPS,
            }
        );
    }

    #[test]
    fn rotating_log_writer_rotates_before_overflow() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("error.log");
        let mut writer =
            RotatingLogWriter::open(&path, LogRotationPolicy::new(10, 2)).expect("open writer");

        writer.write_all(b"12345").expect("write first chunk");
        writer.write_all(b"67890").expect("write to limit");
        writer.write_all(b"abc").expect("write after limit");
        writer.flush().expect("flush");

        assert_eq!(std::fs::read(&path).expect("read active"), b"abc");
        assert_eq!(
            std::fs::read(path.with_file_name("error.log.1")).expect("read backup"),
            b"1234567890"
        );
    }

    #[test]
    fn rotating_log_writer_removes_oldest_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("error.log");
        let mut writer =
            RotatingLogWriter::open(&path, LogRotationPolicy::new(4, 2)).expect("open writer");

        writer.write_all(b"1111").expect("write first chunk");
        writer.write_all(b"2222").expect("write second chunk");
        writer.write_all(b"3333").expect("write third chunk");
        writer.write_all(b"4444").expect("write fourth chunk");
        writer.flush().expect("flush");

        assert_eq!(std::fs::read(&path).expect("read active"), b"4444");
        assert_eq!(
            std::fs::read(path.with_file_name("error.log.1")).expect("read first backup"),
            b"3333"
        );
        assert_eq!(
            std::fs::read(path.with_file_name("error.log.2")).expect("read second backup"),
            b"2222"
        );
    }
}
