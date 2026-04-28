/// 汎用ブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenericBlockedState {
    /// API で原因を特定できないブロック（`mergeStateStatus == "BLOCKED"` かつ他シグナルすべて None）
    BlockedUnknown,
}
