use crate::contexts::evaluation::application::errors::ErrorToken;
use crate::contexts::evaluation::application::prompt::display_item::DisplayItem;
use crate::contexts::evaluation::application::prompt::to_display_items;
use crate::contexts::evaluation::domain::display_config::{
    CompiledDisplayConfig, CompiledTokenConfig, render_error_token, render_token,
};
use crate::contexts::evaluation::domain::prompt::{PrId, Prompt};
use crate::shared::refresh_mode::RefreshMode;

/// `render()` の戻り値。
pub struct RenderResult {
    pub output: String,
    pub refresh_mode: RefreshMode,
    /// PR 別のレンダリング済み文字列。watch 表示用。
    pub pr_outputs: Vec<(PrId, String)>,
}

/// 取得済みの `Prompt`／`ErrorToken` と表示設定だけからレンダリング結果を組み立てる
/// **純関数**。I/O を持たないので、テストはこの関数を直接呼べばよい。
#[must_use]
pub fn render(
    prompt_result: Result<Prompt, ErrorToken>,
    config: &CompiledDisplayConfig,
) -> RenderResult {
    match prompt_result {
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
            let mut items = to_display_items(prs);
            // 純関数として決定的な並びを保証する。PR 番号昇順で走査することで、
            // 集約後のグループは初出（= 最小 ID）順、グループ内 ID は昇順になる。
            items.sort_by_key(|(pr_id, _)| *pr_id);
            let multiple = items.len() > 1;

            // watch 用: PR ごとに ID なしでレンダリングする（行 = PR で読めるため現状維持）。
            let pr_outputs: Vec<(PrId, String)> = items
                .iter()
                .map(|(pr_id, display_items)| (*pr_id, render_watch_items(display_items, config)))
                .collect();

            let refresh_mode = if items.iter().any(|(_, dis)| {
                dis.iter()
                    .any(|i| matches!(i, DisplayItem::CiPending | DisplayItem::StatusCalculating))
            }) {
                RefreshMode::Hot
            } else {
                RefreshMode::Warm
            };

            // prompt 用: 同一ステータス（DisplayItem）を 1 トークンに集約し PR 番号を列挙する。
            let output = group_by_display_item(&items)
                .iter()
                .map(|(item, ids)| {
                    let pr_ids = if multiple {
                        join_pr_ids(ids)
                    } else {
                        String::new()
                    };
                    render_token(item_to_token(*item, config), Some(&pr_ids))
                })
                .collect::<Vec<_>>()
                .join(" ");

            RenderResult {
                output,
                refresh_mode,
                pr_outputs,
            }
        }
        Err(token) => RenderResult {
            output: render_error(&token, config),
            refresh_mode: RefreshMode::Warm,
            pr_outputs: vec![],
        },
    }
}

/// watch 表示用に PR の各ステータストークンを ID なしでレンダリングし、スペース区切りで連結する。
fn render_watch_items(display_items: &[DisplayItem], config: &CompiledDisplayConfig) -> String {
    display_items
        .iter()
        .map(|item| render_token(item_to_token(*item, config), Some("")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `(PrId, Vec<DisplayItem>)` の列を `DisplayItem` ごとに反転集約する。
///
/// `items` が PR 番号昇順であることを前提に、グループは初出（= 最小 ID）順、
/// グループ内の PR 番号は昇順で並ぶ。1 PR が複数ステータスを持つ場合、その番号は
/// 該当する各グループに重複して現れる（情報欠落なし）。
fn group_by_display_item(items: &[(PrId, Vec<DisplayItem>)]) -> Vec<(DisplayItem, Vec<PrId>)> {
    let mut groups: Vec<(DisplayItem, Vec<PrId>)> = Vec::new();
    for (pr_id, display_items) in items {
        for item in display_items {
            if let Some((_, ids)) = groups.iter_mut().find(|(d, _)| d == item) {
                ids.push(*pr_id);
            } else {
                groups.push((*item, vec![*pr_id]));
            }
        }
    }
    groups
}

/// PR 番号を `#1734 #2669` のように `#` 付き・スペース区切りで連結する。
fn join_pr_ids(ids: &[PrId]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn item_to_token(item: DisplayItem, config: &CompiledDisplayConfig) -> &CompiledTokenConfig {
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

fn render_error(token: &ErrorToken, config: &CompiledDisplayConfig) -> String {
    render_error_token(&config.error, &token.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::domain::display_config::DisplayConfig;
    use crate::contexts::evaluation::domain::prompt::{
        PrId, Prompt, PullRequest, State,
        pull_request::state::blocked::{BlockedState, BranchSyncState, CiState},
        pull_request::state::unblocked::UnblockedState,
    };

    fn render_ok(p: Prompt) -> RenderResult {
        render(Ok(p), &DisplayConfig::default().compile())
    }

    // ── RefreshMode 導出 ────────────────────────────────────────────────────

    #[test]
    fn ci_pending_returns_hot() {
        let result = render_ok(Prompt::PullRequests(vec![PullRequest {
            id: PrId::new(1),
            state: State::Blocked(BlockedState {
                branch_sync: None,
                ci: Some(CiState::Pending),
                review: None,
                generic: None,
            }),
        }]));
        assert_eq!(result.refresh_mode, RefreshMode::Hot);
    }

    #[test]
    fn calculating_returns_hot() {
        let result = render_ok(Prompt::PullRequests(vec![PullRequest {
            id: PrId::new(1),
            state: State::Calculating,
        }]));
        assert_eq!(result.refresh_mode, RefreshMode::Hot);
    }

    #[test]
    fn ci_fail_returns_warm() {
        let result = render_ok(Prompt::PullRequests(vec![PullRequest {
            id: PrId::new(1),
            state: State::Blocked(BlockedState {
                branch_sync: None,
                ci: Some(CiState::Fail),
                review: None,
                generic: None,
            }),
        }]));
        assert_eq!(result.refresh_mode, RefreshMode::Warm);
    }

    #[test]
    fn merge_ready_returns_warm() {
        let result = render_ok(Prompt::PullRequests(vec![PullRequest {
            id: PrId::new(1),
            state: State::Unblocked(UnblockedState::MergeReady),
        }]));
        assert_eq!(result.refresh_mode, RefreshMode::Warm);
    }

    #[test]
    fn empty_pull_requests_returns_terminal() {
        let result = render_ok(Prompt::PullRequests(vec![]));
        assert_eq!(result.refresh_mode, RefreshMode::Terminal);
    }

    #[test]
    fn no_pull_request_returns_warm() {
        let result = render_ok(Prompt::NoPullRequest);
        assert_eq!(result.refresh_mode, RefreshMode::Warm);
    }

    #[test]
    fn fetch_error_returns_warm() {
        let result = render(
            Err(ErrorToken {
                message: "unexpected error".to_owned(),
            }),
            &DisplayConfig::default().compile(),
        );
        assert_eq!(result.refresh_mode, RefreshMode::Warm);
    }

    // ── 複数 PR レンダリング ────────────────────────────────────────────────

    #[test]
    fn single_pr_renders_without_pr_id_number() {
        let result = render_ok(Prompt::PullRequests(vec![PullRequest {
            id: PrId::new(200),
            state: State::Unblocked(UnblockedState::MergeReady),
        }]));
        assert_eq!(result.pr_outputs.len(), 1);
        assert!(!result.output.contains('#'));
        assert_eq!(result.output, "✓ Ready for merge");
    }

    #[test]
    fn multiple_prs_render_with_pr_id_numbers() {
        let result = render_ok(Prompt::PullRequests(vec![
            PullRequest {
                id: PrId::new(200),
                state: State::Unblocked(UnblockedState::MergeReady),
            },
            PullRequest {
                id: PrId::new(201),
                state: State::Unblocked(UnblockedState::Draft),
            },
        ]));
        assert_eq!(result.pr_outputs.len(), 2);
        assert!(result.output.contains("200"));
        assert!(result.output.contains("201"));
    }

    #[test]
    fn multiple_prs_pr_outputs_omit_pr_id_numbers_for_watch() {
        let result = render_ok(Prompt::PullRequests(vec![
            PullRequest {
                id: PrId::new(200),
                state: State::Unblocked(UnblockedState::MergeReady),
            },
            PullRequest {
                id: PrId::new(201),
                state: State::Unblocked(UnblockedState::Draft),
            },
        ]));

        assert_eq!(result.pr_outputs[0].1, "✓ Ready for merge");
        assert_eq!(result.pr_outputs[1].1, "✎ Ready for review");
    }

    #[test]
    fn multiple_prs_pr_outputs_keyed_by_pr_id() {
        let result = render_ok(Prompt::PullRequests(vec![
            PullRequest {
                id: PrId::new(200),
                state: State::Unblocked(UnblockedState::MergeReady),
            },
            PullRequest {
                id: PrId::new(201),
                state: State::Unblocked(UnblockedState::Draft),
            },
        ]));
        assert_eq!(result.pr_outputs[0].0, PrId::new(200));
        assert_eq!(result.pr_outputs[1].0, PrId::new(201));
    }

    #[test]
    fn same_status_prs_aggregated_into_single_token() {
        let result = render_ok(Prompt::PullRequests(vec![
            PullRequest {
                id: PrId::new(200),
                state: State::Unblocked(UnblockedState::MergeReady),
            },
            PullRequest {
                id: PrId::new(201),
                state: State::Unblocked(UnblockedState::MergeReady),
            },
        ]));
        assert_eq!(result.output, "✓ Ready for merge #200 #201");
    }

    #[test]
    fn pr_with_multiple_statuses_appears_in_each_group() {
        let result = render_ok(Prompt::PullRequests(vec![
            PullRequest {
                id: PrId::new(1),
                state: State::Blocked(BlockedState {
                    branch_sync: Some(BranchSyncState::Conflict),
                    ci: Some(CiState::Fail),
                    review: None,
                    generic: None,
                }),
            },
            PullRequest {
                id: PrId::new(2),
                state: State::Blocked(BlockedState {
                    branch_sync: None,
                    ci: Some(CiState::Fail),
                    review: None,
                    generic: None,
                }),
            },
        ]));
        assert_eq!(
            result.output,
            "✗ Resolve conflict #1 ✗ Fix CI failure #1 #2"
        );
    }

    #[test]
    fn error_renders_with_message() {
        let config = DisplayConfig::default().compile();
        let token = ErrorToken {
            message: "authentication required".to_owned(),
        };
        assert_eq!(render_error(&token, &config), "✗ authentication required");
    }
}
