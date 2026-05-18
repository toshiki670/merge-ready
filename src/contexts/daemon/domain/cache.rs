mod entry;
mod port;
mod repo_id;

pub use entry::CacheEntryState;
pub use port::CachePort;
pub use repo_id::RepoId;

/// 旧名互換 alias。`CacheEntryState` への段階移行のために残す。
/// Issue #339 の最終 Phase で削除する。
pub type CacheEntry = CacheEntryState;
