/// リポジトリ操作で発生しうるインフラエラー種別（全ドメイントレイト共通）
///
/// infra の実装手段（CLI・REST 等）に依存しない抽象的な分類のみを持つ。
/// ドメインの状態（PR なし・デフォルトブランチ等）は `Prompt` で表現するため含まない。
#[derive(Copy, Clone)]
pub enum RepositoryError {
    /// 認証不可（ツール未インストール・未認証を含む）
    Unauthenticated,
    /// レート制限によりアクセス不可
    RateLimited,
    /// 上記に当てはまらない予期しないエラー
    Unexpected,
}
