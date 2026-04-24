use std::process::ExitCode;

/// デーモンのステータス情報
pub struct DaemonStatus {
    pub pid: u32,
    pub entries: usize,
    pub uptime_secs: u64,
    pub version: String,
}

/// デーモンのライフサイクル管理ポート
pub trait DaemonLifecyclePort {
    /// デーモンを起動する。アイドルタイムアウトまたは Stop リクエストで返る。
    fn start(&self) -> ExitCode;
    /// デーモンを停止する。成功時は `true` を返す。
    fn stop(&self) -> bool;
    /// デーモンのステータスを取得する。起動していない場合は `None` を返す。
    fn get_status(&self) -> Option<DaemonStatus>;
}
