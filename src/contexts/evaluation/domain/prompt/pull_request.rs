pub mod id;
pub mod state;

pub use id::PrId;
pub use state::State;

/// PR 番号と評価状態のペア。
#[derive(Debug)]
pub struct PullRequest {
    pub id: PrId,
    pub state: State,
}
