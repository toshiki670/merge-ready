use crate::domain::error::RepositoryError;

/// PR のライフサイクル状態（外部コマンドの文字列表現に非依存）
pub enum PrLifecycle {
    Open,
    NotOpen,
}

pub fn is_open(lifecycle: &PrLifecycle) -> bool {
    matches!(lifecycle, PrLifecycle::Open)
}

pub trait PrStateRepository {
    fn fetch_lifecycle(&self) -> Result<PrLifecycle, RepositoryError>;
}
