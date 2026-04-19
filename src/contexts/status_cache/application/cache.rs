use crate::contexts::status_cache::domain::{CachePort, CacheState};

/// application 層が外部に公開するキャッシュ問い合わせ結果
///
/// ドメインの [`CacheState`] に加え、インフラ不可用（デーモン未起動など）を表す `Unavailable` を追加する。
pub enum CacheQueryResult {
    /// TTL 内の新鮮なキャッシュ
    Fresh(String),
    /// TTL 超過のキャッシュ（デーモンが内部でリフレッシュを予約済み）
    Stale(String),
    /// キャッシュなし（デーモンが内部でリフレッシュを予約済み）
    Miss,
    /// デーモン未起動またはソケット接続失敗
    Unavailable,
}

/// キャッシュを問い合わせるユースケース
pub fn query(port: &impl CachePort, repo_id: &str) -> CacheQueryResult {
    match port.query(repo_id) {
        Ok(CacheState::Fresh(s)) => CacheQueryResult::Fresh(s),
        Ok(CacheState::Stale(s)) => CacheQueryResult::Stale(s),
        Ok(CacheState::Miss) => CacheQueryResult::Miss,
        Err(()) => CacheQueryResult::Unavailable,
    }
}

/// キャッシュを更新するユースケース
pub fn update(port: &impl CachePort, repo_id: &str, output: &str) {
    port.update(repo_id, output);
}
