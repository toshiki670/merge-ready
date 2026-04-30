/// ロギングのためのエラーカテゴリ（横断的関心事）
pub enum ErrorCategory {
    Auth,
    RateLimit,
    Timeout,
    Unknown,
}

/// ログに記録する構造化エントリ
pub struct LogRecord {
    pub category: ErrorCategory,
    pub detail: Option<String>,
}

/// エラーをログ記録するポート
pub trait ErrorLogger {
    fn log(&self, record: &LogRecord);
}
