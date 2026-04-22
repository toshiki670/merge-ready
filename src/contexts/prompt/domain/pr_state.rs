use super::error::RepositoryError;

/// PR のライフサイクル状態（外部コマンドの文字列表現に非依存）
pub enum PrLifecycle {
    Open,
    NotOpen,
}

#[must_use]
pub fn is_open(lifecycle: &PrLifecycle) -> bool {
    matches!(lifecycle, PrLifecycle::Open)
}

pub trait PrStateRepository {
    /// # Errors
    /// Returns `RepositoryError` if the PR lifecycle cannot be fetched.
    fn fetch_lifecycle(&self) -> Result<PrLifecycle, RepositoryError>;
}
