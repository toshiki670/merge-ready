use serde::{Deserialize, Serialize};

/// キャッシュエントリのリフレッシュ頻度を制御するモード。
///
/// evaluation コンテキストの render 結果と daemon のキャッシュ更新ロジックの
/// 双方が同じ enum を参照するため、IPC ワイヤフォーマット上の表現も兼ねる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshMode {
    /// CI 実行中。素早いリフレッシュが必要。
    Hot,
    /// CI 完了・通常監視中。
    Warm,
    /// PR が merged / closed。リフレッシュ不要。
    Terminal,
}
