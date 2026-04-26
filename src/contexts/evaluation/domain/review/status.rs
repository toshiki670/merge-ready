/// レビュー決定状態（インフラから取得した生の値）
pub enum ReviewStatus {
    ChangesRequested,
    Approved,
    ReviewRequired,
    NoDecision,
}

/// レビューのブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewState {
    ChangesRequested,
}
