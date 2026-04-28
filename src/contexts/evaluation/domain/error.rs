/// リポジトリ操作で発生しうるエラー種別（全ドメイントレイト共通）
///
/// infra の実装手段（CLI・REST 等）に依存しない抽象的な分類のみを持つ。
#[derive(Copy, Clone)]
pub enum RepositoryError {
    /// 認証不可（ツール未インストール・未認証を含む）
    Unauthenticated,
    /// リソースが存在しない（該当 PR なし等）
    NotFound,
    /// レート制限によりアクセス不可
    RateLimited,
    /// 上記に当てはまらない予期しないエラー
    Unexpected,
}
