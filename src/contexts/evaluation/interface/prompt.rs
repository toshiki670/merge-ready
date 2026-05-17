use crate::contexts::evaluation::application::errors::ErrorToken;
use crate::contexts::evaluation::application::port::ErrorLogger;
use crate::contexts::evaluation::application::prompt::display_item::DisplayItem;
use crate::contexts::evaluation::application::prompt::{fetch, to_display_items};
use crate::contexts::evaluation::domain::display_config::{
    DisplayConfig, DisplayConfigRepository, TokenConfig, render_error_token, render_token,
};
use crate::contexts::evaluation::domain::prompt::{PrId, Prompt, PromptRepository};
use crate::shared::refresh_mode::RefreshMode;

/// `render()` の戻り値。
pub struct RenderResult {
    pub output: String,
    pub refresh_mode: RefreshMode,
    /// PR 別のレンダリング済み文字列。watch 表示用。
    pub pr_outputs: Vec<(PrId, String)>,
}

pub fn render<R, C, L>(repo: &R, config_repo: &C, logger: &L) -> RenderResult
where
    R: PromptRepository,
    C: DisplayConfigRepository,
    L: ErrorLogger,
{
    let config = config_repo.load();
    match fetch(repo, logger) {
        Ok(Prompt::NoRepository | Prompt::UnsupportedRepository | Prompt::DefaultBranch) => {
            RenderResult {
                output: String::new(),
                refresh_mode: RefreshMode::Warm,
                pr_outputs: vec![],
            }
        }
        Ok(Prompt::NoPullRequest) => RenderResult {
            output: render_token(&config.no_pull_request, None),
            refresh_mode: RefreshMode::Warm,
            pr_outputs: vec![],
        },
        Ok(ref p) if p.is_terminal() => RenderResult {
            output: String::new(),
            refresh_mode: RefreshMode::Terminal,
            pr_outputs: vec![],
        },
        Ok(Prompt::PullRequests(prs)) => {
            let items = to_display_items(prs);
            let show_pr_id = items.len() > 1;
            let mut pr_outputs = Vec::new();
            let mut all_outputs = Vec::new();

            for (pr_id, display_items) in &items {
                let id_str = if show_pr_id {
                    pr_id.to_string()
                } else {
                    String::new()
                };
                let prompt_out = render_display_items(display_items, &config, &id_str);
                let watch_out = render_display_items(display_items, &config, "");
                pr_outputs.push((*pr_id, watch_out));
                all_outputs.push(prompt_out);
            }

            let refresh_mode = if items.iter().any(|(_, dis)| {
                dis.iter()
                    .any(|i| matches!(i, DisplayItem::CiPending | DisplayItem::StatusCalculating))
            }) {
                RefreshMode::Hot
            } else {
                RefreshMode::Warm
            };

            RenderResult {
                output: all_outputs.join(" "),
                refresh_mode,
                pr_outputs,
            }
        }
        Err(token) => RenderResult {
            output: render_error(&token, &config),
            refresh_mode: RefreshMode::Warm,
            pr_outputs: vec![],
        },
    }
}

fn render_display_items(
    display_items: &[DisplayItem],
    config: &DisplayConfig,
    pr_id: &str,
) -> String {
    display_items
        .iter()
        .map(|item| render_token(item_to_token(item, config), Some(pr_id)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn item_to_token<'a>(item: &DisplayItem, config: &'a DisplayConfig) -> &'a TokenConfig {
    match item {
        DisplayItem::MergeReady => &config.merge_ready,
        DisplayItem::Conflict => &config.conflict,
        DisplayItem::UpdateBranch => &config.update_branch,
        DisplayItem::SyncUnknown => &config.sync_unknown,
        DisplayItem::CiFail => &config.ci_fail,
        DisplayItem::CiAction => &config.ci_action,
        DisplayItem::CiPending => &config.ci_pending,
        DisplayItem::ChangesRequested => &config.changes_requested,
        DisplayItem::ReviewRequired => &config.review_required,
        DisplayItem::Draft => &config.draft,
        DisplayItem::StatusCalculating => &config.status_calculating,
        DisplayItem::BlockedUnknown => &config.blocked_unknown,
    }
}

fn render_error(token: &ErrorToken, config: &DisplayConfig) -> String {
    render_error_token(&config.error, &token.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::domain::display_config::DisplayConfig;
    use crate::contexts::evaluation::domain::error::RepositoryError;
    use crate::contexts::evaluation::domain::prompt::{
        PrId, Prompt, PromptRepository, PullRequest, State,
        pull_request::state::blocked::{BlockedState, ci::CiState},
        pull_request::state::unblocked::UnblockedState,
    };

    struct StubRepoFn(fn() -> Result<Prompt, RepositoryError>);
    impl PromptRepository for StubRepoFn {
        fn fetch(&self) -> Result<Prompt, RepositoryError> {
            (self.0)()
        }
    }

    struct NoOpLogger;
    impl crate::contexts::evaluation::application::port::ErrorLogger for NoOpLogger {
        fn log(&self, _: &crate::contexts::evaluation::application::port::LogRecord) {}
    }

    struct NoOpConfigRepo;
    impl DisplayConfigRepository for NoOpConfigRepo {
        fn load(&self) -> DisplayConfig {
            DisplayConfig::default()
        }
    }

    fn do_render(f: fn() -> Result<Prompt, RepositoryError>) -> RenderResult {
        render(&StubRepoFn(f), &NoOpConfigRepo, &NoOpLogger)
    }

    // ── RefreshMode 導出 ────────────────────────────────────────────────────

    #[test]
    fn ci_pending_returns_hot() {
        let result = do_render(|| {
            Ok(Prompt::PullRequests(vec![PullRequest {
                id: PrId::new(1),
                state: State::Blocked(BlockedState {
                    branch_sync: None,
                    ci: Some(CiState::Pending),
                    review: None,
                    generic: None,
                }),
            }]))
        });
        assert_eq!(result.refresh_mode, RefreshMode::Hot);
    }

    #[test]
    fn calculating_returns_hot() {
        let result = do_render(|| {
            Ok(Prompt::PullRequests(vec![PullRequest {
                id: PrId::new(1),
                state: State::Calculating,
            }]))
        });
        assert_eq!(result.refresh_mode, RefreshMode::Hot);
    }

    #[test]
    fn ci_fail_returns_warm() {
        let result = do_render(|| {
            Ok(Prompt::PullRequests(vec![PullRequest {
                id: PrId::new(1),
                state: State::Blocked(BlockedState {
                    branch_sync: None,
                    ci: Some(CiState::Fail),
                    review: None,
                    generic: None,
                }),
            }]))
        });
        assert_eq!(result.refresh_mode, RefreshMode::Warm);
    }

    #[test]
    fn merge_ready_returns_warm() {
        let result = do_render(|| {
            Ok(Prompt::PullRequests(vec![PullRequest {
                id: PrId::new(1),
                state: State::Unblocked(UnblockedState::MergeReady),
            }]))
        });
        assert_eq!(result.refresh_mode, RefreshMode::Warm);
    }

    #[test]
    fn empty_pull_requests_returns_terminal() {
        let result = do_render(|| Ok(Prompt::PullRequests(vec![])));
        assert_eq!(result.refresh_mode, RefreshMode::Terminal);
    }

    #[test]
    fn no_pull_request_returns_warm() {
        let result = do_render(|| Ok(Prompt::NoPullRequest));
        assert_eq!(result.refresh_mode, RefreshMode::Warm);
    }

    #[test]
    fn fetch_error_returns_warm() {
        let result = do_render(|| Err(RepositoryError::Unexpected));
        assert_eq!(result.refresh_mode, RefreshMode::Warm);
    }

    // ── 複数 PR レンダリング ────────────────────────────────────────────────

    #[test]
    fn single_pr_renders_without_pr_id_number() {
        let result = do_render(|| {
            Ok(Prompt::PullRequests(vec![PullRequest {
                id: PrId::new(200),
                state: State::Unblocked(UnblockedState::MergeReady),
            }]))
        });
        assert_eq!(result.pr_outputs.len(), 1);
        assert!(!result.output.contains('#'));
        assert_eq!(result.output, "✓ Ready for merge");
    }

    #[test]
    fn multiple_prs_render_with_pr_id_numbers() {
        let result = do_render(|| {
            Ok(Prompt::PullRequests(vec![
                PullRequest {
                    id: PrId::new(200),
                    state: State::Unblocked(UnblockedState::MergeReady),
                },
                PullRequest {
                    id: PrId::new(201),
                    state: State::Unblocked(UnblockedState::Draft),
                },
            ]))
        });
        assert_eq!(result.pr_outputs.len(), 2);
        assert!(result.output.contains("200"));
        assert!(result.output.contains("201"));
    }

    #[test]
    fn multiple_prs_pr_outputs_omit_pr_id_numbers_for_watch() {
        let result = do_render(|| {
            Ok(Prompt::PullRequests(vec![
                PullRequest {
                    id: PrId::new(200),
                    state: State::Unblocked(UnblockedState::MergeReady),
                },
                PullRequest {
                    id: PrId::new(201),
                    state: State::Unblocked(UnblockedState::Draft),
                },
            ]))
        });

        assert_eq!(result.pr_outputs[0].1, "✓ Ready for merge");
        assert_eq!(result.pr_outputs[1].1, "✎ Ready for review");
    }

    #[test]
    fn multiple_prs_pr_outputs_keyed_by_pr_id() {
        let result = do_render(|| {
            Ok(Prompt::PullRequests(vec![
                PullRequest {
                    id: PrId::new(200),
                    state: State::Unblocked(UnblockedState::MergeReady),
                },
                PullRequest {
                    id: PrId::new(201),
                    state: State::Unblocked(UnblockedState::Draft),
                },
            ]))
        });
        assert_eq!(result.pr_outputs[0].0, PrId::new(200));
        assert_eq!(result.pr_outputs[1].0, PrId::new(201));
    }

    #[test]
    fn error_renders_with_message() {
        let config = DisplayConfig::default();
        let token = ErrorToken {
            message: "authentication required".to_owned(),
        };
        assert_eq!(render_error(&token, &config), "✗ authentication required");
    }
}
