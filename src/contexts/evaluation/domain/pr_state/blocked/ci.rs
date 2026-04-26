/// CI のブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiState {
    Fail,
    ActionRequired,
}
