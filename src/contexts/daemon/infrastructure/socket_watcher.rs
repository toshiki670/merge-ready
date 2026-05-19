//! 自身の socket ファイルの存在を周期的にチェックする独立タスク。
//!
//! テストハーネスが異常終了して `daemon stop` が走らず `TempDir` ごと消えるケースで
//! 孤児化しないよう、socket が外部から削除されたら `CancellationToken::cancel`
//! して daemon 全体の停止を要求する。

use std::sync::Arc;
use std::time::Duration;

use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use super::paths::Paths;

pub(super) async fn run(paths: Arc<Paths>, interval_secs: u64, cancel: CancellationToken) {
    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // 起動直後の即時 tick を捨てる。
    ticker.tick().await;

    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            _ = ticker.tick() => {
                if !paths.socket_path().exists() {
                    log::info!("daemon socket disappeared, self-terminating");
                    cancel.cancel();
                    break;
                }
            }
        }
    }
}
