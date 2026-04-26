/// チェック全体の集約状態（インフラから取得した生の値）
pub enum CiStatus {
    Fail,
    ActionRequired,
    Pass,
}

/// CI のブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiState {
    Fail,
    ActionRequired,
}
