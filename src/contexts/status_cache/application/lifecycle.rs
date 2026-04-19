use crate::contexts::status_cache::domain::{DaemonLifecyclePort, DaemonStatus};

// interface 層が domain を直接参照しないよう re-export する
pub use crate::contexts::status_cache::domain::DaemonLifecyclePort as Port;

/// デーモンを起動するユースケース
pub fn start(port: &impl DaemonLifecyclePort) {
    port.start();
}

/// デーモンを停止するユースケース。成功時は `true` を返す。
pub fn stop(port: &impl DaemonLifecyclePort) -> bool {
    port.stop()
}

/// デーモンのステータスを取得するユースケース
pub fn get_status(port: &impl DaemonLifecyclePort) -> Option<DaemonStatus> {
    port.get_status()
}
