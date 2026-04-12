/// リポジトリ操作で発生しうるエラー種別（全ドメイントレイト共通）
pub enum RepositoryError {
    NotInstalled,
    AuthRequired,
    NoPr,
    RateLimited,
    ApiError(String),
}
