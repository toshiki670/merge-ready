use crate::contexts::status_cache::domain::daemon::{DaemonLifecyclePort, DaemonStatus};

pub trait Port: DaemonLifecyclePort {}
impl<T> Port for T where T: DaemonLifecyclePort {}

/// デーモンを起動するユースケース
pub fn start(port: &impl Port) {
    port.start();
}

/// デーモンを停止するユースケース。成功時は `true` を返す。
pub fn stop(port: &impl Port) -> bool {
    port.stop()
}

/// デーモンのステータスを取得するユースケース
pub fn get_status(port: &impl Port) -> Option<DaemonStatus> {
    port.get_status()
}
