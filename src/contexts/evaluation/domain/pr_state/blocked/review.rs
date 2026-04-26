/// レビューのブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewState {
    /// レビュアーが変更を要求している
    ChangesRequested,
}
