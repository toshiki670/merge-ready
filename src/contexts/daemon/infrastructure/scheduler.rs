//! `tokio::time::interval` で `DaemonStateHandle::tick` を周期的に呼び、
//! 返ってきた `Effect` を drain する scheduler タスク。
//!
//! `SpawnRefresh` は `on_refresh` 経由で `tokio::spawn` に流す。
//! `RecordExpired` は `log::debug!`、他の `Effect` は scheduler では無視する。

use std::time::{Duration, Instant, SystemTime};

use tokio::runtime::Handle;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use super::daemon_server::{RefreshFn, spawn_refresh};
use super::daemon_state_actor::DaemonStateHandle;
use crate::contexts::daemon::domain::cache::Effect;

pub(super) async fn run(
    state_handle: DaemonStateHandle,
    on_refresh: RefreshFn,
    handle: Handle,
    tick_secs: u64,
    cancel: CancellationToken,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(tick_secs));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // 起動直後の即時 tick を捨てる（現状の `recv_timeout` ベースが「最初に tick_secs 待つ」
    // 仕様だったので振る舞いを揃える）。
    ticker.tick().await;

    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            _ = ticker.tick() => {
                let effects = state_handle
                    .tick(Instant::now(), SystemTime::now())
                    .await;
                for e in effects {
                    match e {
                        Effect::SpawnRefresh { repo_id, cwd } => {
                            spawn_refresh(&repo_id, &cwd, on_refresh, &handle);
                        }
                        Effect::RecordExpired { repo_id } => {
                            log::debug!("entry expired: {repo_id:?}");
                        }
                        Effect::EmitOutput(_) | Effect::EnterBackoff { .. } => {}
                    }
                }
            }
        }
    }
}
