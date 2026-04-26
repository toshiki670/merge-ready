/// CI のブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiState {
    /// チェックが失敗またはキャンセルされている
    Fail,
    /// 手動アクションが必要なチェックが存在する
    ActionRequired,
}
