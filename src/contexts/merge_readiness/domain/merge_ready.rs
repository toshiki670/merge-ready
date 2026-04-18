use super::error::RepositoryError;

/// マージ可否に必要な状態（外部コマンドの文字列表現に非依存）
pub struct MergeReadiness {
    pub is_draft: bool,
    /// ブランチ保護ルールを全て満たしている（`CLEAN` または `HAS_HOOKS` に相当）
    pub is_protected: bool,
}

#[must_use]
pub fn is_ready(readiness: &MergeReadiness) -> bool {
    !readiness.is_draft && readiness.is_protected
}

pub trait MergeReadinessRepository {
    /// # Errors
    /// Returns `RepositoryError` if the merge readiness cannot be fetched.
    fn fetch_readiness(&self) -> Result<MergeReadiness, RepositoryError>;
}
