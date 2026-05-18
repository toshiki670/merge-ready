use std::path::PathBuf;
use std::time::Instant;

use super::repo_id::RepoId;

/// `transition` モジュールの純粋関数が返す副作用要求。
///
/// `daemon` の各 edge (`connection` ハンドラ / scheduler / `rate_limit` fetcher)
/// が drain して実際の副作用（リフレッシュ起動、ソケット書き込み、ログ）を行う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// バックグラウンドリフレッシュを起動する。
    SpawnRefresh { repo_id: RepoId, cwd: PathBuf },
    /// クライアントへ返す出力文字列（Query レスポンス）。
    EmitOutput(String),
    /// 期限切れエントリを削除した通知（ログ用途）。
    /// 戻り値の `CacheStore` 内部では既に削除済み。
    RecordExpired { repo_id: RepoId },
    /// Rate limit 枯渇/閾値超過を観測し、`until` まで全リフレッシュを停止する。
    /// 戻り値の `CacheStore` 内部では既に `backoff_until` に反映済み（ログ・通知用途）。
    EnterBackoff { until: Instant },
}
