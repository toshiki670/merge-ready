pub mod pull_request;

pub use pull_request::PullRequest;
pub use pull_request::id::PrId;
pub use pull_request::state::State;

/// プロンプト表示の集約ルート。ブランチの PR 状況を表す。
#[derive(Debug)]
pub enum Prompt {
    /// Git リポジトリではない
    NoRepository,
    /// サポートされていないリポジトリ（GitHub 以外・リモートなし）
    UnsupportedRepository,
    /// デフォルトブランチ上
    DefaultBranch,
    /// PR が作られていない
    NoPullRequest,
    /// PR の集合（空 Vec はターミナル状態：全 PR がマージ済み/クローズ済み）
    PullRequests(Vec<PullRequest>),
}

impl Prompt {
    /// 全 PR がクローズ済みでデーモンのポーリングを停止すべき状態かどうかを返す。
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Prompt::PullRequests(v) if v.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_repository_is_not_terminal() {
        assert!(!Prompt::NoRepository.is_terminal());
    }

    #[test]
    fn no_pull_request_is_not_terminal() {
        assert!(!Prompt::NoPullRequest.is_terminal());
    }

    #[test]
    fn empty_pull_requests_is_terminal() {
        assert!(Prompt::PullRequests(vec![]).is_terminal());
    }

    #[test]
    fn non_empty_pull_requests_is_not_terminal() {
        use pull_request::state::unblocked::UnblockedState;
        let pr = PullRequest {
            id: PrId::new(1),
            state: State::Unblocked(UnblockedState::MergeReady),
        };
        assert!(!Prompt::PullRequests(vec![pr]).is_terminal());
    }
}
