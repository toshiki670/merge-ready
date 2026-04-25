/// キャッシュの更新ポート
pub trait CachePort {
    /// キャッシュを更新する。失敗は静かに無視する。
    fn update(&self, repo_id: &str, output: &str);
}
