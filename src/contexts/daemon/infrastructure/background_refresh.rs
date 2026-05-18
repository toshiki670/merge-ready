use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use super::server_state::DaemonState;
use crate::contexts::daemon::domain::cache::{
    Effect, RepoId, SchedulerTickInput, on_scheduler_tick,
};

pub(super) fn collect_targets(state: &Arc<Mutex<DaemonState>>) -> Vec<(RepoId, PathBuf)> {
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let config = s.config;
    let policy = config.policy;

    // rate_limit_aware が OFF のとき snapshot を強制的に None として扱う必要がある。
    // 簡単のため、snapshot 参照を作らず、policy 経由で扱う代わりに、
    // on_scheduler_tick の入力では snapshot を直接渡せないので、
    // OFF のとき CacheStore::latest_rate_limit() を一時的に隠す方法はない。
    // ここでは config.rate_limit_aware を見て、tick 関数の入力から外す。
    // Step 5/6 でこの分岐を SchedulerTickInput 自体に持たせるか整理する。
    // 暫定: tick 関数内では常に store.latest_rate_limit() を使う。
    // rate_limit_aware=false のときは latest_rate_limit を None に保つ運用で対応する
    // （rate_limit fetcher スレッド自体が起動しないので latest_rate_limit は永遠に None）。
    let input = SchedulerTickInput {
        now: Instant::now(),
        now_wall: SystemTime::now(),
        policy: &policy,
        refresh_lock_timeout_secs: config.refresh_lock_timeout_secs,
        entry_max_age_secs: config.entry_max_age_secs,
    };

    let (new_store, effects) = on_scheduler_tick(&s.cache_store, &input);
    s.cache_store = new_store;

    let mut targets = Vec::new();
    for e in effects {
        match e {
            Effect::SpawnRefresh { repo_id, cwd } => targets.push((repo_id, cwd)),
            Effect::RecordExpired { repo_id } => log::debug!("entry expired: {repo_id:?}"),
            Effect::EmitOutput(_) | Effect::EnterBackoff { .. } => {}
        }
    }
    targets
}

// 単体テストは transition::on_scheduler_tick の test モジュールに集約されている
// （backoff スキップ・期限切れクリア・interval 経過 spawn の振る舞いはすべて
// 純粋関数側で網羅）。本ファイルは scheduler スレッドの edge 配線（Mutex の
// lock とロック後の transition 呼び出し）のみを担う。
