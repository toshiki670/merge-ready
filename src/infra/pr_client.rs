use crate::domain::{
    branch_sync::BranchSyncStatus, ci_checks::CheckBucket, merge_ready::MergeReadiness,
    pr_state::PrLifecycle, review::ReviewStatus,
};

/// `gh pr view` から取得した全ドメイン状態（翻訳済み）
pub struct PrViewData {
    pub lifecycle: PrLifecycle,
    pub sync_status: BranchSyncStatus,
    pub review_status: ReviewStatus,
    pub merge_readiness: MergeReadiness,
}

/// PR クライアントのエラー種別
pub enum PrClientError {
    NotInstalled,
    AuthRequired,
    NoPr,
    RateLimited,
    ApiError(String),
}

/// PR データ取得の共通インターフェース（`gh` / `glab` を差し替え可能にする）
pub trait PrClient {
    fn pr_view(&self) -> Result<PrViewData, PrClientError>;
    /// `gh pr checks` の結果を [`CheckBucket`] のリストに翻訳して返す
    fn pr_checks(&self) -> Result<Vec<CheckBucket>, PrClientError>;
}
