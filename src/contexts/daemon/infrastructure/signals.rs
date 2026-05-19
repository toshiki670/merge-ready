//! `tokio::signal::unix` で SIGTERM/SIGINT を購読し、
//! 最初のシグナル受信で `CancellationToken::cancel` を呼ぶ独立タスク。
//!
//! `JoinSet` 上で他の async タスクと並列に走らせ、外部からの停止要求と
//! `Request::Stop` 経由の停止要求を同一の `cancel` 経路へ収束させる。

use tokio::signal::unix::{SignalKind, signal};
use tokio_util::sync::CancellationToken;

pub(super) async fn install_shutdown_signals(cancel: CancellationToken) {
    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => {
            log::error!("failed to install SIGTERM handler: {e}");
            return;
        }
    };
    let mut sigint = match signal(SignalKind::interrupt()) {
        Ok(s) => s,
        Err(e) => {
            log::error!("failed to install SIGINT handler: {e}");
            return;
        }
    };
    tokio::select! {
        _ = sigterm.recv() => log::info!("SIGTERM received, shutting down"),
        _ = sigint.recv() => log::info!("SIGINT received, shutting down"),
        () = cancel.cancelled() => return,
    }
    cancel.cancel();
}
