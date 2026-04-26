use crate::contexts::evaluation::domain::error::RepositoryError;

/// マージ可否に必要な状態（外部コマンドの文字列表現に非依存）
pub struct MergeReadiness {
    pub is_draft: bool,
    /// ブランチ保護ルールを全て満たしている（`CLEAN` または `HAS_HOOKS` に相当）
    pub is_protected: bool,
}

/// PR がブロックされていないときの評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnblockedState {
    /// `is_draft=false` && `is_protected=true`
    MergeReady,
    /// `is_draft=true`（ready-for-review）
    Draft,
}

impl MergeReadiness {
    #[must_use]
    pub fn to_unblocked_state(&self) -> Option<UnblockedState> {
        if self.is_draft {
            Some(UnblockedState::Draft)
        } else if self.is_protected {
            Some(UnblockedState::MergeReady)
        } else {
            None
        }
    }
}

pub trait UnblockedRepository {
    /// # Errors
    /// Returns `RepositoryError` if the merge readiness cannot be fetched.
    fn fetch_readiness(&self) -> Result<MergeReadiness, RepositoryError>;
}
