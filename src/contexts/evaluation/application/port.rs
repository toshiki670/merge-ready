use super::super::domain::branch_sync::BranchSyncRepository;
use super::super::domain::ci_checks::CiChecksRepository;
use super::super::domain::merge_ready::MergeReadinessRepository;
use super::super::domain::pr_state::PrStateRepository;
use super::super::domain::review::ReviewRepository;

/// prompt ユースケースが必要とするドメインリポジトリを束ねた集約トレイト。
///
/// 個別リポジトリ trait を実装すれば自動的に満たされる。
pub trait PromptStatusPort:
    PrStateRepository
    + BranchSyncRepository
    + CiChecksRepository
    + ReviewRepository
    + MergeReadinessRepository
{
}

impl<T> PromptStatusPort for T where
    T: PrStateRepository
        + BranchSyncRepository
        + CiChecksRepository
        + ReviewRepository
        + MergeReadinessRepository
{
}
