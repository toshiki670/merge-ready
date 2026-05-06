pub mod display_item;

use display_item::DisplayItem;

use super::errors::{ErrorToken, into_token};
use super::port::ErrorLogger;
use crate::contexts::evaluation::domain::prompt::{PrId, Prompt, PromptRepository};

/// PR 状態を取得するユースケース。
/// インフラエラーをロギングして `ErrorToken` に変換し、ドメイン状態はそのまま返す。
pub fn fetch<R, L>(repo: &R, logger: &L) -> Result<Prompt, ErrorToken>
where
    R: PromptRepository,
    L: ErrorLogger,
{
    repo.fetch().map_err(|e| into_token(e, logger))
}

/// `Prompt::PullRequests` の各エントリを `(PrId, Vec<DisplayItem>)` に変換する。
pub fn to_display_items(
    prs: Vec<crate::contexts::evaluation::domain::prompt::PullRequest>,
) -> Vec<(PrId, Vec<DisplayItem>)> {
    prs.into_iter()
        .map(|pr| (pr.id, display_item::from_state(pr.state)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::domain::error::RepositoryError;
    use crate::contexts::evaluation::domain::prompt::{
        PrId, Prompt, PromptRepository, PullRequest, State,
        pull_request::state::unblocked::UnblockedState,
    };

    struct NoOpLogger;
    impl crate::contexts::evaluation::application::port::ErrorLogger for NoOpLogger {
        fn log(&self, _: &crate::contexts::evaluation::application::port::LogRecord) {}
    }

    struct StubRepo(fn() -> Result<Prompt, RepositoryError>);
    impl PromptRepository for StubRepo {
        fn fetch(&self) -> Result<Prompt, RepositoryError> {
            (self.0)()
        }
    }

    #[test]
    fn no_pull_request_passes_through() {
        let repo = StubRepo(|| Ok(Prompt::NoPullRequest));
        let result = fetch(&repo, &NoOpLogger).unwrap();
        assert!(matches!(result, Prompt::NoPullRequest));
    }

    #[test]
    fn no_repository_passes_through() {
        let repo = StubRepo(|| Ok(Prompt::NoRepository));
        let result = fetch(&repo, &NoOpLogger).unwrap();
        assert!(matches!(result, Prompt::NoRepository));
    }

    #[test]
    fn pull_requests_passes_through() {
        let repo = StubRepo(|| {
            Ok(Prompt::PullRequests(vec![PullRequest {
                id: PrId::new(42),
                state: State::Unblocked(UnblockedState::MergeReady),
            }]))
        });
        let result = fetch(&repo, &NoOpLogger).unwrap();
        let Prompt::PullRequests(prs) = result else {
            panic!("expected PullRequests");
        };
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].id, PrId::new(42));
    }

    #[test]
    fn infra_error_returns_error_token() {
        let repo = StubRepo(|| Err(RepositoryError::Unauthenticated));
        let result = fetch(&repo, &NoOpLogger);
        assert!(result.is_err());
    }

    #[test]
    fn to_display_items_maps_states() {
        let prs = vec![PullRequest {
            id: PrId::new(1),
            state: State::Unblocked(UnblockedState::MergeReady),
        }];
        let items = to_display_items(prs);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, PrId::new(1));
        assert_eq!(items[0].1.len(), 1);
        assert!(matches!(items[0].1[0], DisplayItem::MergeReady));
    }
}
