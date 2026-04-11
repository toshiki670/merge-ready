/// PR のライフサイクル状態（外部コマンドの文字列表現に非依存）
pub enum PrLifecycle {
    Open,
    NotOpen,
}

pub fn is_open(lifecycle: &PrLifecycle) -> bool {
    matches!(lifecycle, PrLifecycle::Open)
}
