pub mod display_item;

use display_item::DisplayItem;

use super::errors::{ErrorToken, into_token};
use super::port::ErrorLogger;
use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::pr_state::{PrId, PrRepository, entries_are_terminal};

/// `fetch()` の戻り値。PR なし・PR あり（複数可）を区別する。
pub enum FetchResult {
    /// 非 GitHub リポジトリ・デフォルトブランチなど、表示不要な状態。
    NotApplicable,
    NoPullRequest,
    Entries {
        items: Vec<(PrId, Vec<DisplayItem>)>,
        is_terminal: bool,
    },
}

/// PR 状態を取得するユースケース。
/// `NotFound` は表示不要な状態として `NoPullRequest` を返す。
pub fn fetch<R, L>(repo: &R, logger: &L) -> Result<FetchResult, ErrorToken>
where
    R: PrRepository,
    L: ErrorLogger,
{
    let entries = match repo.fetch() {
        Ok(e) => e,
        Err(RepositoryError::NotFound) => return Ok(FetchResult::NoPullRequest),
        Err(RepositoryError::NotGithubRepository) => return Ok(FetchResult::NotApplicable),
        Err(e) => match into_token(e, logger) {
            Some(token) => return Err(token),
            None => unreachable!(
                "into_token returns None only for NotFound/NotGithubRepository, handled above"
            ),
        },
    };

    if entries.is_empty() {
        return Ok(FetchResult::NoPullRequest);
    }

    let is_terminal = entries_are_terminal(&entries);
    let items = entries
        .into_iter()
        .map(|e| (e.pr_id, display_item::from_pr_state(e.state)))
        .collect();

    Ok(FetchResult::Entries { items, is_terminal })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::domain::error::RepositoryError;
    use crate::contexts::evaluation::domain::pr_state::{
        PrEntry, PrId, PrRepository, PrState, entries_are_terminal,
        not_applicable::NotApplicableState, unblocked::UnblockedState,
    };

    struct NoOpLogger;
    impl crate::contexts::evaluation::application::port::ErrorLogger for NoOpLogger {
        fn log(&self, _: &crate::contexts::evaluation::application::port::LogRecord) {}
    }

    struct StubRepo(Vec<PrEntry>);
    impl PrRepository for StubRepo {
        fn fetch(&self) -> Result<Vec<PrEntry>, RepositoryError> {
            // PrEntry は Copy でないので clone が必要 — テスト用に state だけ再構築
            unreachable!("use StubRepoFn instead")
        }
    }

    struct StubRepoVec(fn() -> Vec<PrEntry>);
    impl PrRepository for StubRepoVec {
        fn fetch(&self) -> Result<Vec<PrEntry>, RepositoryError> {
            Ok((self.0)())
        }
    }

    struct ErrRepo(RepositoryError);
    impl PrRepository for ErrRepo {
        fn fetch(&self) -> Result<Vec<PrEntry>, RepositoryError> {
            Err(self.0)
        }
    }

    #[test]
    fn empty_entries_returns_no_pull_request() {
        let repo = StubRepoVec(|| vec![]);
        let result = fetch(&repo, &NoOpLogger).unwrap();
        assert!(matches!(result, FetchResult::NoPullRequest));
    }

    #[test]
    fn not_found_error_returns_no_pull_request() {
        let repo = ErrRepo(RepositoryError::NotFound);
        let result = fetch(&repo, &NoOpLogger).unwrap();
        assert!(matches!(result, FetchResult::NoPullRequest));
    }

    #[test]
    fn not_github_repo_error_returns_not_applicable() {
        let repo = ErrRepo(RepositoryError::NotGithubRepository);
        let result = fetch(&repo, &NoOpLogger).unwrap();
        assert!(matches!(result, FetchResult::NotApplicable));
    }

    #[test]
    fn single_entry_returns_entries_with_items() {
        let repo = StubRepoVec(|| {
            vec![PrEntry {
                pr_id: PrId::new(42),
                state: PrState::Unblocked(UnblockedState::MergeReady),
            }]
        });
        let result = fetch(&repo, &NoOpLogger).unwrap();
        let FetchResult::Entries { items, is_terminal } = result else {
            panic!("expected Entries");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, PrId::new(42));
        assert!(!is_terminal);
    }

    #[test]
    fn multiple_entries_returns_all_items() {
        let repo = StubRepoVec(|| {
            vec![
                PrEntry {
                    pr_id: PrId::new(200),
                    state: PrState::Unblocked(UnblockedState::MergeReady),
                },
                PrEntry {
                    pr_id: PrId::new(201),
                    state: PrState::Unblocked(UnblockedState::Draft),
                },
            ]
        });
        let result = fetch(&repo, &NoOpLogger).unwrap();
        let FetchResult::Entries { items, .. } = result else {
            panic!("expected Entries");
        };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, PrId::new(200));
        assert_eq!(items[1].0, PrId::new(201));
    }

    #[test]
    fn terminal_entries_sets_is_terminal() {
        let repo = StubRepoVec(|| {
            vec![PrEntry {
                pr_id: PrId::new(1),
                state: PrState::NotApplicable(NotApplicableState::Merged),
            }]
        });
        let result = fetch(&repo, &NoOpLogger).unwrap();
        let FetchResult::Entries { is_terminal, .. } = result else {
            panic!("expected Entries");
        };
        assert!(is_terminal);
    }
}
