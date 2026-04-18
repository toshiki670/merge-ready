use crate::contexts::merge_readiness::application::{
    BranchSyncRepository, CiChecksRepository, MergeReadinessRepository, PrStateRepository,
    ReviewRepository, errors::ErrorLogger,
};
use crate::contexts::merge_readiness::interface::presentation::{PresentationConfigPort, Presenter};

pub fn run<C, L, P>(client: &C, logger: &L, config_port: P)
where
    C: PrStateRepository
        + BranchSyncRepository
        + CiChecksRepository
        + ReviewRepository
        + MergeReadinessRepository
        + Sync,
    L: ErrorLogger + Sync,
    P: PresentationConfigPort + Sync,
{
    let presenter = Presenter::new(config_port);
    let tokens = crate::contexts::merge_readiness::application::run(client, logger, &presenter);
    if !tokens.is_empty() {
        presenter.display(&tokens);
    }
}
