use crate::contexts::merge_readiness::domain::signal::PromptSignal;

/// マージ可否に必要な状態（外部コマンドの文字列表現に非依存）
pub struct MergeReadiness {
    pub is_draft: bool,
    /// ブランチ保護ルールを全て満たしている（`CLEAN` または `HAS_HOOKS` に相当）
    pub is_protected: bool,
}

impl MergeReadiness {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        !self.is_draft && self.is_protected
    }

    #[must_use]
    pub fn signal(&self) -> Option<PromptSignal> {
        if self.is_ready() {
            Some(PromptSignal::MergeReady)
        } else {
            None
        }
    }
}
