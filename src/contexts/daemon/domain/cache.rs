/// daemon がキャッシュエントリのリフレッシュ頻度を制御するモード。
/// evaluation コンテキストの知識（CI 状態・終端状態）を抽象化した daemon 固有の概念。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshMode {
    /// CI 実行中。素早いリフレッシュが必要。
    Hot,
    /// CI 完了・通常監視中。
    Warm,
    /// PR が merged / closed。リフレッシュ不要。
    Terminal,
}

/// キャッシュの更新ポート
pub trait CachePort {
    /// キャッシュを更新する。失敗は静かに無視する。
    fn update(&self, repo_id: &str, output: &str, refresh_mode: RefreshMode);
}
