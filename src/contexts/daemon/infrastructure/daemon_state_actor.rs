//! `DaemonState` を単一の tokio タスクが所有するアクター。
//!
//! 外部からは `DaemonStateHandle` のメソッドを通じてアクセスする。各メソッドは
//! mpsc 経由でコマンドを送り、oneshot で返信を受け取る async fn。Mutex を取らない
//! ため、ロック順序由来のバグが構造的に発生しない。

use std::time::{Instant, SystemTime};

use tokio::sync::{mpsc, oneshot};

use super::request_handler::{self, ActionResult};
use super::server_config::DaemonServerConfig;
use super::server_state::DaemonState;
use crate::contexts::daemon::domain::cache::{
    Effect, RateLimitObservedEvent, SchedulerTickInput, on_rate_limit_observed, on_scheduler_tick,
};
use crate::shared::protocol::Request;

const COMMAND_CHANNEL_SIZE: usize = 64;

pub(super) enum DaemonCommand {
    /// `Request` を `request_handler::process` に通して `ActionResult` を返す。
    Process {
        request: Request,
        reply: oneshot::Sender<ActionResult>,
    },
    /// `on_scheduler_tick` を呼び、生の `Effect` 列を返す。drain は呼び出し側の責務。
    Tick {
        now: Instant,
        now_wall: SystemTime,
        reply: oneshot::Sender<Vec<Effect>>,
    },
    /// `on_rate_limit_observed` を呼び、生の `Effect` 列を返す。drain は呼び出し側の責務。
    ApplyRateLimit {
        event: RateLimitObservedEvent,
        reply: oneshot::Sender<Vec<Effect>>,
    },
}

/// アクターへの送信ハンドル。`Clone` でタスク間に複製してよい。
#[derive(Clone)]
pub(super) struct DaemonStateHandle {
    tx: mpsc::Sender<DaemonCommand>,
}

impl DaemonStateHandle {
    pub(super) async fn process(&self, request: Request) -> Option<ActionResult> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DaemonCommand::Process { request, reply: tx })
            .await
            .ok()?;
        rx.await.ok()
    }

    pub(super) async fn tick(&self, now: Instant, now_wall: SystemTime) -> Vec<Effect> {
        let (tx, rx) = oneshot::channel();
        if self
            .tx
            .send(DaemonCommand::Tick {
                now,
                now_wall,
                reply: tx,
            })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }

    pub(super) async fn apply_rate_limit(&self, event: RateLimitObservedEvent) -> Vec<Effect> {
        let (tx, rx) = oneshot::channel();
        if self
            .tx
            .send(DaemonCommand::ApplyRateLimit { event, reply: tx })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }
}

/// アクターを `tokio::spawn` で起動し、送信ハンドルを返す。
pub(super) fn spawn(config: DaemonServerConfig) -> DaemonStateHandle {
    let (tx, rx) = mpsc::channel(COMMAND_CHANNEL_SIZE);
    tokio::spawn(run_actor(config, rx));
    DaemonStateHandle { tx }
}

async fn run_actor(config: DaemonServerConfig, mut rx: mpsc::Receiver<DaemonCommand>) {
    let mut state = DaemonState::new(config);
    while let Some(cmd) = rx.recv().await {
        handle_command(&mut state, cmd);
    }
}

fn handle_command(state: &mut DaemonState, cmd: DaemonCommand) {
    match cmd {
        DaemonCommand::Process { request, reply } => {
            let config = state.config;
            let result = request_handler::process(
                &request,
                &mut state.cache_store,
                &config.policy,
                state.started_at,
                config.stale_ttl_secs,
            );
            let _ = reply.send(result);
        }
        DaemonCommand::Tick {
            now,
            now_wall,
            reply,
        } => {
            let config = state.config;
            let input = SchedulerTickInput {
                now,
                now_wall,
                policy: &config.policy,
                refresh_lock_timeout_secs: config.refresh_lock_timeout_secs,
                entry_max_age_secs: config.entry_max_age_secs,
            };
            let (new_store, effects) = on_scheduler_tick(&state.cache_store, &input);
            state.cache_store = new_store;
            let _ = reply.send(effects);
        }
        DaemonCommand::ApplyRateLimit { event, reply } => {
            let (new_store, effects) = on_rate_limit_observed(&state.cache_store, &event);
            state.cache_store = new_store;
            let _ = reply.send(effects);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::protocol::Response;

    fn test_config() -> DaemonServerConfig {
        DaemonServerConfig::from_env()
    }

    #[tokio::test]
    async fn process_status_returns_zero_entries_at_startup() {
        let handle = spawn(test_config());
        let result = handle.process(Request::Status).await.expect("actor reply");
        let Response::Status { entries, .. } = result.response else {
            panic!("expected Response::Status");
        };
        assert_eq!(entries, 0);
    }

    #[tokio::test]
    async fn process_entries_returns_empty_vec_at_startup() {
        let handle = spawn(test_config());
        let result = handle.process(Request::Entries).await.expect("actor reply");
        let Response::Entries { entries } = result.response else {
            panic!("expected Response::Entries");
        };
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn process_stop_sets_stop_flag() {
        let handle = spawn(test_config());
        let result = handle.process(Request::Stop).await.expect("actor reply");
        assert!(result.stop);
    }

    #[tokio::test]
    async fn tick_returns_empty_effects_when_no_entries() {
        let handle = spawn(test_config());
        let effects = handle.tick(Instant::now(), SystemTime::now()).await;
        assert!(effects.is_empty());
    }

    #[tokio::test]
    async fn apply_rate_limit_emits_backoff_when_exhausted() {
        use crate::contexts::daemon::domain::rate_limit_snapshot::RateLimitSnapshot;

        let handle = spawn(test_config());
        let now_wall = SystemTime::now();
        let snapshot = RateLimitSnapshot {
            core_remaining: 0,
            core_limit: 5000,
            graphql_remaining: 5000,
            graphql_limit: 5000,
            reset_at: now_wall + std::time::Duration::from_mins(1),
            fetched_at: Instant::now(),
        };
        let event = RateLimitObservedEvent {
            snapshot,
            now: Instant::now(),
            now_wall,
        };
        let effects = handle.apply_rate_limit(event).await;
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::EnterBackoff { .. })),
            "expected EnterBackoff effect"
        );
    }
}
