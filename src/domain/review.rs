/// レビュー決定状態
pub enum ReviewStatus {
    ChangesRequested,
    Other,
}

/// `review` 評価（独立条件）
pub fn evaluate(status: &ReviewStatus) -> Option<&'static str> {
    match status {
        ReviewStatus::ChangesRequested => Some("⚠ review"),
        ReviewStatus::Other => None,
    }
}
