/// ドメイン上のキャッシュ状態（インフラ可用性を含まない純粋なドメイン概念）
pub enum CacheState {
    /// TTL 内の新鮮なキャッシュ
    Fresh(String),
    /// TTL 超過のキャッシュ（デーモンが内部でリフレッシュを予約済み）
    Stale(String),
    /// キャッシュなし（デーモンが内部でリフレッシュを予約済み）
    Miss,
}

/// キャッシュの問い合わせ・更新ポート
pub trait CachePort {
    /// キャッシュを問い合わせる。デーモン未起動など接続失敗時は `Err(())` を返す。
    fn query(&self, repo_id: &str) -> Result<CacheState, ()>;
    /// キャッシュを更新する。失敗は静かに無視する。
    fn update(&self, repo_id: &str, output: &str);
}

/// デーモンのステータス情報
pub struct DaemonStatus {
    pub pid: u32,
    pub entries: usize,
    pub uptime_secs: u64,
}

/// デーモンのライフサイクル管理ポート
pub trait DaemonLifecyclePort {
    /// デーモンを起動する。通常は返らない（`std::process::exit` で終了）。
    fn start(&self);
    /// デーモンを停止する。成功時は `true` を返す。
    fn stop(&self) -> bool;
    /// デーモンのステータスを取得する。起動していない場合は `None` を返す。
    fn get_status(&self) -> Option<DaemonStatus>;
}
