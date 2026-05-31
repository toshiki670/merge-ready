pub mod display_item;

use display_item::DisplayItem;

use crate::contexts::evaluation::domain::prompt::{PrId, PullRequest};

/// `Prompt::PullRequests` の各エントリを `(PrId, Vec<DisplayItem>)` に変換する。
pub fn to_display_items(prs: Vec<PullRequest>) -> Vec<(PrId, Vec<DisplayItem>)> {
    prs.into_iter()
        .map(|pr| (pr.id, display_item::from_state(pr.state)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::domain::prompt::{
        PrId, PullRequest, State, pull_request::state::unblocked::UnblockedState,
    };
    use std::assert_matches;

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
        assert_matches!(items[0].1[0], DisplayItem::MergeReady);
    }
}
