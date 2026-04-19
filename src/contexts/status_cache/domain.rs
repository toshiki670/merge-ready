/// デーモンへのキャッシュ問い合わせ結果
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
