use super::super::domain::pr_state::PrStateRepository;
use super::super::domain::pr_state::blocked::branch_sync::BranchSyncRepository;
use super::super::domain::pr_state::blocked::ci::CiChecksRepository;
use super::super::domain::pr_state::blocked::review::ReviewRepository;
use super::super::domain::pr_state::unblocked::UnblockedRepository;

/// prompt ユースケースが必要とするドメインリポジトリを束ねた集約トレイト。
///
/// 個別リポジトリ trait を実装すれば自動的に満たされる。
pub trait PromptStatusPort:
    PrStateRepository
    + BranchSyncRepository
    + CiChecksRepository
    + ReviewRepository
    + UnblockedRepository
{
}

impl<T> PromptStatusPort for T where
    T: PrStateRepository
        + BranchSyncRepository
        + CiChecksRepository
        + ReviewRepository
        + UnblockedRepository
{
}
