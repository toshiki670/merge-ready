use merge_readiness_application::{
    BranchSyncRepository, CiChecksRepository, MergeReadinessRepository, PrStateRepository,
    ReviewRepository, errors::ErrorLogger,
};

use crate::presentation;

/// gh を直接呼んで結果を stdout に出力する（キャッシュを使わない）。
pub fn run<C, L>(client: &C, logger: &L)
where
    C: PrStateRepository
        + BranchSyncRepository
        + CiChecksRepository
        + ReviewRepository
        + MergeReadinessRepository
        + Sync,
    L: ErrorLogger + Sync,
{
    let presenter = presentation::Presenter;
    let tokens = merge_readiness_application::run(client, logger, &presenter);
    if !tokens.is_empty() {
        presentation::display(&tokens);
    }
}
