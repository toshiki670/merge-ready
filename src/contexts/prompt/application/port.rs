use super::super::domain::branch_sync::BranchSync;
use super::super::domain::ci_checks::CiChecks;
use super::super::domain::error::RepositoryError;
use super::super::domain::merge_ready::MergeReadiness;
use super::super::domain::pr_state::PrLifecycle;
use super::super::domain::review::Review;

/// PR の状態を収集するユースケース向け集約ポート。
///
/// 1 つのユースケース（prompt 出力生成）内で必要な 5 つのフェッチ責務を束ね、
/// ドメイン内部のリポジトリ trait を外に漏らさない。
pub trait PromptStatusPort {
    /// # Errors
    /// Returns `RepositoryError` if the PR lifecycle cannot be fetched.
    fn fetch_lifecycle(&self) -> Result<PrLifecycle, RepositoryError>;

    /// # Errors
    /// Returns `RepositoryError` if the sync status cannot be fetched.
    fn fetch_sync_status(&self) -> Result<BranchSync, RepositoryError>;

    /// # Errors
    /// Returns `RepositoryError` if the CI checks cannot be fetched.
    fn fetch_checks(&self) -> Result<CiChecks, RepositoryError>;

    /// # Errors
    /// Returns `RepositoryError` if the review state cannot be fetched.
    fn fetch_review(&self) -> Result<Review, RepositoryError>;

    /// # Errors
    /// Returns `RepositoryError` if the merge readiness cannot be fetched.
    fn fetch_readiness(&self) -> Result<MergeReadiness, RepositoryError>;
}
