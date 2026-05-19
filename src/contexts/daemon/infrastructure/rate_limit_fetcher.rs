//! `gh api rate_limit` を周期的に取得し、`DaemonStateHandle::apply_rate_limit`
//! でアクターに反映する async タスク。
//!
//! 取得呼び出しの実時間が `interval` を超えても catch-up burst にならないよう
//! `MissedTickBehavior::Delay` を選び、完了から `interval` 待つセマンティクスを得る。
//! 返ってきた `Effect::EnterBackoff` のみログに出力する。

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use super::daemon_state_actor::DaemonStateHandle;
use super::rate_limit_client::RateLimitClient;
use crate::contexts::daemon::domain::cache::{Effect, RateLimitObservedEvent};

pub(super) async fn run(
    state_handle: DaemonStateHandle,
    client: Arc<RateLimitClient>,
    interval: Duration,
    cancel: CancellationToken,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            _ = ticker.tick() => {
                if let Some(snapshot) = client.fetch_or_cached().await {
                    let event = RateLimitObservedEvent {
                        snapshot,
                        now: Instant::now(),
                        now_wall: SystemTime::now(),
                    };
                    let effects = state_handle.apply_rate_limit(event).await;
                    for e in effects {
                        if let Effect::EnterBackoff { until } = e {
                            log::info!("rate_limit backoff until {until:?}");
                        }
                    }
                }
            }
        }
    }
}
