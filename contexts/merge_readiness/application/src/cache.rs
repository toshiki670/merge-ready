/// キャッシュの状態（アプリケーション層の表現）
pub enum CacheState {
    Fresh(String),
    Stale(String),
    Miss,
}

/// キャッシュ読み取りを抽象化するポート
pub trait CachePort {
    fn check(&self, repo_id: &str) -> CacheState;
}

/// main に伝える表示アクション
pub(crate) enum DisplayAction {
    /// そのまま表示する（バックグラウンドリフレッシュ不要）
    Display(String),
    /// 表示してからバックグラウンドリフレッシュを要求する
    DisplayAndRefresh(String),
    /// "? loading" を表示してバックグラウンドリフレッシュを要求する（キャッシュミス）
    LoadingWithRefresh,
}

/// キャッシュ方針に基づいて表示アクションを決定する
pub(crate) fn resolve(repo_id: &str, cache: &impl CachePort) -> DisplayAction {
    match cache.check(repo_id) {
        CacheState::Fresh(s) => DisplayAction::Display(s),
        CacheState::Stale(s) => DisplayAction::DisplayAndRefresh(s),
        CacheState::Miss => DisplayAction::LoadingWithRefresh,
    }
}
