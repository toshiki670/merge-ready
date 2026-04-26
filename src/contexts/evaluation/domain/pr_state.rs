use super::error::RepositoryError;

pub trait PrStateRepository {
    /// # Errors
    /// Returns `RepositoryError` if the PR lifecycle cannot be fetched.
    fn fetch_lifecycle(&self) -> Result<PrLifecycle, RepositoryError>;
}

/// PR のライフサイクル状態（外部コマンドの文字列表現に非依存）
pub enum PrLifecycle {
    Open,
    Merged,
    Closed,
}

#[must_use]
pub fn is_open(lifecycle: &PrLifecycle) -> bool {
    matches!(lifecycle, PrLifecycle::Open)
}

impl PrLifecycle {
    /// クローズ / マージ済みで追跡不要な状態かどうかを返す。
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, PrLifecycle::Merged | PrLifecycle::Closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_is_not_terminal() {
        assert!(!PrLifecycle::Open.is_terminal());
    }

    #[test]
    fn merged_is_terminal() {
        assert!(PrLifecycle::Merged.is_terminal());
    }

    #[test]
    fn closed_is_terminal() {
        assert!(PrLifecycle::Closed.is_terminal());
    }
}
