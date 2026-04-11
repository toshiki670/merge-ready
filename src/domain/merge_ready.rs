/// マージ可否に必要な状態（外部コマンドの文字列表現に非依存）
pub struct MergeReadiness {
    pub is_draft: bool,
    /// ブランチ保護ルールを全て満たしている（`CLEAN` または `HAS_HOOKS` に相当）
    pub is_protected: bool,
}

pub fn is_ready(readiness: &MergeReadiness) -> bool {
    !readiness.is_draft && readiness.is_protected
}
