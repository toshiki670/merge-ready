use crate::contexts::daemon::domain::daemon::{DaemonError, DaemonLifecyclePort, DaemonStatus};

pub trait Port: DaemonLifecyclePort {}
impl<T> Port for T where T: DaemonLifecyclePort {}

/// デーモンを起動するユースケース
pub fn start(port: &impl Port) -> Result<(), DaemonError> {
    port.start()
}

/// デーモンを停止するユースケース。成功時は `true` を返す。
pub fn stop(port: &impl Port) -> bool {
    port.stop()
}

/// デーモンのステータスを取得するユースケース
pub fn get_status(port: &impl Port) -> Option<DaemonStatus> {
    port.get_status()
}

/// 実行中デーモンの PID を取得するユースケース
pub fn get_pid(port: &impl Port) -> Option<u32> {
    port.get_pid()
}
