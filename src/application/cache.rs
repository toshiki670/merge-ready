/// キャッシュの状態（アプリケーション層の表現）
pub(crate) enum CacheState {
    Fresh(String),
    Stale(String),
    Miss,
}

/// キャッシュ読み取りを抽象化するポート
pub(crate) trait CachePort {
    fn check(&self, repo_id: &str) -> CacheState;
}

/// main に伝える表示アクション
pub(crate) enum DisplayAction {
    /// そのまま表示する（バックグラウンドリフレッシュ不要）
    Display(String),
    /// 表示してからバックグラウンドリフレッシュを要求する
    DisplayAndRefresh(String),
    /// "? loading" を表示してバックグラウンドリフレッシュを要求する
    Loading,
}

/// キャッシュ方針に基づいて表示アクションを決定する
pub(crate) fn resolve(repo_id: Option<&str>, cache: &impl CachePort) -> DisplayAction {
    let Some(id) = repo_id else {
        return DisplayAction::Loading;
    };
    match cache.check(id) {
        CacheState::Fresh(s) => DisplayAction::Display(s),
        CacheState::Stale(s) => DisplayAction::DisplayAndRefresh(s),
        CacheState::Miss => DisplayAction::Loading,
    }
}
