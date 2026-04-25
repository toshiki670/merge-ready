/// リポジトリ操作で発生しうるエラー種別（全ドメイントレイト共通）
///
/// infra の実装手段（CLI・REST 等）に依存しない抽象的な分類のみを持つ。
pub enum RepositoryError {
    /// 認証不可（ツール未インストール・未認証を含む）
    Unauthenticated,
    /// リソースが存在しない（該当 PR なし等）
    NotFound,
    /// レート制限によりアクセス不可
    RateLimited,
    /// 上記に当てはまらない予期しないエラー
    Unexpected(String),
}

/// ログ記録のためのエラーカテゴリ（原因調査パス）
#[allow(dead_code)]
pub enum ErrorCategory {
    Auth,
    RateLimit,
    Timeout,
    Unknown,
}

/// ログに記録する構造化エントリ（原因調査パス）
pub struct LogRecord {
    pub category: ErrorCategory,
    /// `Unknown` 系のみ raw メッセージを保持する
    pub detail: Option<String>,
}
