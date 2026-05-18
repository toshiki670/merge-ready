//! `CacheStore` の純粋遷移関数。
//!
//! daemon の各 edge (`request_handler` / `background_refresh` /
//! `daemon_server::update_state_from_snapshot`) は、ロック取得後に
//! 本モジュールの純粋関数を呼んで `(CacheStore, Vec<Effect>)` を得る。
//! 戻り値の `CacheStore` を `Mutex` 配下の値に書き戻し、`Vec<Effect>` を
//! drain して副作用（リフレッシュ起動、ソケット書き込み、ログ）を実行する。

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

use super::effect::Effect;
use super::entry::CacheEntryState;
use super::event::{QueryEvent, RateLimitObservedEvent, RefreshCompletedEvent, SchedulerTickInput};
use super::repo_id::RepoId;
use super::store::CacheStore;
use crate::contexts::daemon::domain::rate_limit_snapshot::RateLimitSnapshot;
use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;
use crate::shared::refresh_mode::RefreshMode;

/// `bottleneck_ratio` の basis points スケール（10000 == 1.0）。
const RATIO_SCALE_BP: u64 = 10_000;
/// ボトルネック残量比率がこの値（basis points）以下になったら backoff へ。
const BACKOFF_THRESHOLD_BP: u64 = 500; // 5%

// ───────────────────────────────────────────────────────────────
// 純粋関数: on_query
// ───────────────────────────────────────────────────────────────

/// Query イベントの純粋遷移。
///
/// 振る舞いは旧 `process_query` / `process_stale_query` と等価。
/// - 未知の `repo_id` → 新規 Loading エントリ作成 + `SpawnRefresh` + `EmitOutput("? loading")`
/// - Fresh → `record_query` 反映 + `EmitOutput(stored)`
/// - Stale + Refreshing + !has_fetched → `EmitOutput("? loading")`
/// - Stale + Refreshing + has_fetched → `EmitOutput(stored)`
/// - Stale + NeedsRefresh → `reset_cold_count`(if cold) + `record_query`
///   + `reset_to_warm`(if Terminal) + `mark_refreshing`
///   + `SpawnRefresh` + `EmitOutput(stored)`
#[must_use]
pub fn on_query(
    store: &CacheStore,
    event: QueryEvent,
    repo_id: &RepoId,
    policy: &RefreshPolicy,
) -> (CacheStore, Vec<Effect>) {
    let mut entries = store.entries().clone();
    let mut effects: Vec<Effect> = Vec::with_capacity(2);

    match entries.get(repo_id) {
        Some(entry)
            if is_fresh(
                entry,
                policy.effective_ttl(entry, event.stale_ttl),
                event.now,
            ) =>
        {
            let new_entry = entry.clone().with_record_query(event.now);
            let output = new_entry.output().to_owned();
            entries.insert(repo_id.clone(), new_entry);
            effects.push(Effect::EmitOutput(output));
        }
        Some(entry) => {
            let stored_output = entry.output().to_owned();
            let has_fetched = entry.has_fetched();
            let stored_cwd = entry.cwd().to_path_buf();
            let is_refreshing = entry.is_refreshing();
            let is_terminal = entry.refresh_mode() == RefreshMode::Terminal;
            let was_cold = is_cold_or_never_queried(entry, policy.warm_to_cold_secs, event.now);

            let mut state = entry.clone();
            if was_cold {
                state = state.with_reset_cold_count();
            }
            state = state.with_record_query(event.now);

            if is_refreshing {
                let output = if has_fetched {
                    stored_output
                } else {
                    "? loading".to_owned()
                };
                effects.push(Effect::EmitOutput(output));
            } else {
                if is_terminal {
                    state = state.with_reset_to_warm();
                }
                state = state.with_mark_refreshing(event.now);
                effects.push(Effect::EmitOutput(stored_output));
                effects.push(Effect::SpawnRefresh {
                    repo_id: repo_id.clone(),
                    cwd: stored_cwd,
                });
            }
            entries.insert(repo_id.clone(), state);
        }
        None => {
            let new_entry = CacheEntryState::new_loading(
                event.cwd.clone(),
                event.branch,
                event.stale_ttl,
                event.now,
                event.now_wall,
            );
            entries.insert(repo_id.clone(), new_entry);
            effects.push(Effect::EmitOutput("? loading".to_owned()));
            effects.push(Effect::SpawnRefresh {
                repo_id: repo_id.clone(),
                cwd: event.cwd,
            });
        }
    }

    (store.clone().with_entries(entries), effects)
}

// ───────────────────────────────────────────────────────────────
// 純粋関数: on_refresh_completed
// ───────────────────────────────────────────────────────────────

/// バックグラウンドリフレッシュ完了時の純粋遷移。
///
/// 未知の `repo_id`（ブランチ切替直後の再導出 ID など）は無視する。
/// 既存仕様: `cwd: PathBuf::new()` / `last_queried_at: None` の孤立エントリが
/// 生まれるのを防ぐため。
#[must_use]
pub fn on_refresh_completed(
    store: &CacheStore,
    repo_id: &RepoId,
    event: RefreshCompletedEvent,
) -> (CacheStore, Vec<Effect>) {
    let mut entries = store.entries().clone();
    if let Some(state) = entries.get(repo_id) {
        let new_state = state.clone().with_refresh_completed(
            event.output,
            event.pr_outputs,
            event.refresh_mode,
            event.now,
            event.now_wall,
        );
        entries.insert(repo_id.clone(), new_state);
    }
    (store.clone().with_entries(entries), Vec::new())
}

// ───────────────────────────────────────────────────────────────
// 純粋関数: on_scheduler_tick
// ───────────────────────────────────────────────────────────────

/// スケジューラ tick の純粋遷移。
///
/// 振る舞いは旧 `collect_targets` と等価:
/// - 期限切れ backoff のクリア
/// - backoff 中なら entries 不変・effects 空
/// - 期限切れエントリ削除 + `RecordExpired`
/// - active かつ refresh lock 切れなら `clear_refresh_lock`
/// - active かつ interval 経過なら `mark_refreshing` + `SpawnRefresh`
/// - Warm かつ cold 圏なら `increment_cold_count`
#[must_use]
pub fn on_scheduler_tick(
    store: &CacheStore,
    input: SchedulerTickInput<'_>,
) -> (CacheStore, Vec<Effect>) {
    let now = input.now;

    // 期限切れ backoff のクリア
    let backoff_until = match store.backoff_until() {
        Some(t) if now >= t => None,
        other => other,
    };
    let cleared_store = store.clone().with_backoff_until(backoff_until);

    // backoff 中はリフレッシュを一切行わない
    if cleared_store.is_backed_off(now) {
        return (cleared_store, Vec::new());
    }

    let mut entries = cleared_store.entries().clone();
    let mut effects: Vec<Effect> = Vec::new();

    // 期限切れエントリ削除
    entries.retain(|repo_id, entry| {
        if is_expired(entry, input.entry_max_age_secs, now) {
            effects.push(Effect::RecordExpired {
                repo_id: repo_id.clone(),
            });
            false
        } else {
            true
        }
    });

    let total_cost = total_refresh_cost(entries.values());
    let snapshot = cleared_store.latest_rate_limit().copied();

    let mut updated: HashMap<RepoId, CacheEntryState> = HashMap::with_capacity(entries.len());
    for (repo_id, entry) in entries {
        let mut e = entry;
        if !e.is_active() {
            updated.insert(repo_id, e);
            continue;
        }
        if e.is_refreshing() && refresh_lock_expired(&e, input.refresh_lock_timeout_secs, now) {
            e = e.with_clear_refresh_lock();
        }
        if e.is_refreshing() {
            updated.insert(repo_id, e);
            continue;
        }
        let interval = input.policy.effective_refresh_interval_secs_scaled(
            &e,
            snapshot.as_ref(),
            total_cost,
            input.now,
            input.now_wall,
        );
        if fetched_at_elapsed(&e, now).as_secs() < interval {
            updated.insert(repo_id, e);
            continue;
        }
        if e.refresh_mode() == RefreshMode::Warm && is_cold(&e, input.policy.warm_to_cold_secs, now)
        {
            e = e.with_increment_cold_count();
        }
        let cwd = e.cwd().to_path_buf();
        e = e.with_mark_refreshing(now);
        effects.push(Effect::SpawnRefresh {
            repo_id: repo_id.clone(),
            cwd,
        });
        updated.insert(repo_id, e);
    }

    (cleared_store.with_entries(updated), effects)
}

fn total_refresh_cost<'a, I: IntoIterator<Item = &'a CacheEntryState>>(entries: I) -> u64 {
    let mut total: u64 = 0;
    for e in entries {
        if !e.is_active() {
            continue;
        }
        let pr_count = u64::try_from(e.pr_outputs().len()).unwrap_or(u64::MAX);
        total = total.saturating_add(pr_count.saturating_add(2));
    }
    total
}

// ───────────────────────────────────────────────────────────────
// 純粋関数: on_rate_limit_observed
// ───────────────────────────────────────────────────────────────

/// Rate limit スナップショット観測時の純粋遷移。
///
/// 振る舞いは旧 `update_state_from_snapshot` と等価:
/// - `latest_rate_limit` を更新
/// - ボトルネック残量が枯渇または閾値以下なら、reset 時刻まで `backoff_until` を設定し
///   `EnterBackoff` Effect を発行（既存 backoff と新規 backoff の時刻が異なる場合のみ）
#[must_use]
pub fn on_rate_limit_observed(
    store: &CacheStore,
    event: RateLimitObservedEvent,
) -> (CacheStore, Vec<Effect>) {
    let mut effects: Vec<Effect> = Vec::new();
    let snapshot = event.snapshot;

    let new_backoff_until = if should_enter_backoff(&snapshot) {
        reset_instant_from_snapshot(&snapshot, event.now, event.now_wall)
            .or_else(|| store.backoff_until())
    } else {
        store.backoff_until()
    };

    if let Some(until) = new_backoff_until
        && should_enter_backoff(&snapshot)
        && store.backoff_until() != Some(until)
    {
        effects.push(Effect::EnterBackoff { until });
    }

    let new_store = store
        .clone()
        .with_latest_rate_limit(Some(snapshot))
        .with_backoff_until(new_backoff_until);

    (new_store, effects)
}

fn should_enter_backoff(snapshot: &RateLimitSnapshot) -> bool {
    if snapshot.is_exhausted() {
        return true;
    }
    let core_bp = ratio_bp(snapshot.core_remaining, snapshot.core_limit);
    let graphql_bp = ratio_bp(snapshot.graphql_remaining, snapshot.graphql_limit);
    core_bp.min(graphql_bp) <= BACKOFF_THRESHOLD_BP
}

fn ratio_bp(remaining: u32, limit: u32) -> u64 {
    if limit == 0 {
        return RATIO_SCALE_BP;
    }
    (u64::from(remaining).saturating_mul(RATIO_SCALE_BP)) / u64::from(limit)
}

/// `snapshot.reset_at`（壁時計）を `Instant`（モノトニック）へ変換する。
fn reset_instant_from_snapshot(
    snapshot: &RateLimitSnapshot,
    now_instant: Instant,
    now_wall: SystemTime,
) -> Option<Instant> {
    let delta = snapshot.reset_at.duration_since(now_wall).ok()?;
    Some(now_instant + delta)
}

// ───────────────────────────────────────────────────────────────
// 純粋述語ヘルパ（旧 getter の `now` 引数化版）。daemon::domain 内で共有する
// （`RefreshPolicy` も同じヘルパを呼ぶ）。
// ───────────────────────────────────────────────────────────────

pub(in crate::contexts::daemon::domain) fn is_fresh(
    s: &CacheEntryState,
    ttl: u64,
    now: Instant,
) -> bool {
    elapsed_secs(s.fetched_at(), now) <= ttl
}

pub(in crate::contexts::daemon::domain) fn is_expired(
    s: &CacheEntryState,
    max_age_secs: u64,
    now: Instant,
) -> bool {
    s.last_queried_at()
        .is_some_and(|t| elapsed_secs(t, now) >= max_age_secs)
}

pub(in crate::contexts::daemon::domain) fn is_cold(
    s: &CacheEntryState,
    warm_to_cold_secs: u64,
    now: Instant,
) -> bool {
    s.last_queried_at()
        .is_some_and(|t| elapsed_secs(t, now) >= warm_to_cold_secs)
}

pub(in crate::contexts::daemon::domain) fn is_cold_or_never_queried(
    s: &CacheEntryState,
    warm_to_cold_secs: u64,
    now: Instant,
) -> bool {
    s.last_queried_at()
        .is_none_or(|t| elapsed_secs(t, now) >= warm_to_cold_secs)
}

pub(in crate::contexts::daemon::domain) fn has_recent_query(
    s: &CacheEntryState,
    recent_secs: u64,
    now: Instant,
) -> bool {
    s.last_queried_at()
        .is_some_and(|t| elapsed_secs(t, now) <= recent_secs)
}

pub(in crate::contexts::daemon::domain) fn refresh_lock_expired(
    s: &CacheEntryState,
    timeout_secs: u64,
    now: Instant,
) -> bool {
    s.refresh_started_at()
        .is_some_and(|t| elapsed_secs(t, now) >= timeout_secs)
}

pub(in crate::contexts::daemon::domain) fn fetched_at_elapsed(
    s: &CacheEntryState,
    now: Instant,
) -> Duration {
    now.saturating_duration_since(s.fetched_at())
}

fn elapsed_secs(t: Instant, now: Instant) -> u64 {
    now.saturating_duration_since(t).as_secs()
}

// ───────────────────────────────────────────────────────────────
// テスト
// ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use super::super::entry::FetchState;
    use super::*;

    fn test_policy() -> RefreshPolicy {
        RefreshPolicy {
            hot_recent_query_secs: 30,
            hot_with_query_secs: 2,
            hot_without_query_secs: 10,
            warm_refresh_secs: 180,
            warm_to_cold_secs: 1800,
            cold_early_secs: 1800,
            cold_late_secs: 3600,
            cold_early_limit: 10,
        }
    }

    fn query_event(now: Instant, now_wall: SystemTime) -> QueryEvent {
        QueryEvent {
            cwd: PathBuf::from("/repo/main"),
            branch: "main".to_owned(),
            now,
            now_wall,
            stale_ttl: 10,
        }
    }

    /// `update` 済みのエントリ（Ready）を作成。
    fn ready_entry(now: Instant, now_wall: SystemTime, mode: RefreshMode) -> CacheEntryState {
        let mut e = CacheEntryState::new_loading(
            PathBuf::from("/repo/main"),
            "main".to_owned(),
            10,
            now,
            now_wall,
        );
        e = e.with_refresh_completed("hello".to_owned(), Vec::new(), mode, now, now_wall);
        e
    }

    #[test]
    fn on_query_initial_miss_creates_loading_and_spawns_refresh() {
        let store = CacheStore::new();
        let policy = test_policy();
        let repo_id = RepoId::new("repo".to_owned());
        let now = Instant::now();
        let now_wall = SystemTime::now();

        let (new_store, effects) = on_query(&store, query_event(now, now_wall), &repo_id, &policy);

        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0], Effect::EmitOutput("? loading".to_owned()));
        assert!(matches!(effects[1], Effect::SpawnRefresh { .. }));
        assert_eq!(new_store.entries().len(), 1);
        let entry = new_store.entries().get(&repo_id).expect("entry present");
        assert_eq!(entry.fetch_state(), FetchState::Loading);
    }

    #[test]
    fn on_query_fresh_emits_output_only() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let entry = ready_entry(now, now_wall, RefreshMode::Warm);
        let repo_id = RepoId::new("repo".to_owned());
        let mut entries = std::collections::HashMap::new();
        entries.insert(repo_id.clone(), entry);
        let store = CacheStore::new().with_entries(entries);
        let policy = test_policy();

        // 3 秒後 (ttl=10 以内 = fresh)
        let later = now + Duration::from_secs(3);
        let (new_store, effects) =
            on_query(&store, query_event(later, now_wall), &repo_id, &policy);

        assert_eq!(effects, vec![Effect::EmitOutput("hello".to_owned())]);
        let new_entry = new_store.entries().get(&repo_id).expect("entry present");
        assert_eq!(new_entry.last_queried_at(), Some(later));
    }

    #[test]
    fn on_query_stale_needs_refresh_marks_and_spawns() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let entry = ready_entry(now, now_wall, RefreshMode::Warm);
        let repo_id = RepoId::new("repo".to_owned());
        let mut entries = std::collections::HashMap::new();
        entries.insert(repo_id.clone(), entry);
        let store = CacheStore::new().with_entries(entries);
        let policy = test_policy();

        // 100 秒後（stale）
        let later = now + Duration::from_secs(100);
        let (new_store, effects) =
            on_query(&store, query_event(later, now_wall), &repo_id, &policy);

        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0], Effect::EmitOutput("hello".to_owned()));
        assert!(matches!(effects[1], Effect::SpawnRefresh { .. }));
        let new_entry = new_store.entries().get(&repo_id).expect("entry present");
        assert_eq!(new_entry.fetch_state(), FetchState::Refreshing);
    }

    #[test]
    fn on_query_stale_refreshing_no_fetched_returns_loading() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        // Loading 状態（new_loading 直後 = Loading + has_fetched=false）
        let entry = CacheEntryState::new_loading(
            PathBuf::from("/repo/main"),
            "main".to_owned(),
            10,
            now,
            now_wall,
        );
        let repo_id = RepoId::new("repo".to_owned());
        let mut entries = std::collections::HashMap::new();
        entries.insert(repo_id.clone(), entry);
        let store = CacheStore::new().with_entries(entries);
        let policy = test_policy();

        let later = now + Duration::from_secs(100);
        let (_new_store, effects) =
            on_query(&store, query_event(later, now_wall), &repo_id, &policy);

        assert_eq!(effects, vec![Effect::EmitOutput("? loading".to_owned())]);
    }

    #[test]
    fn on_query_stale_refreshing_has_fetched_returns_stored() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        // Refreshing 状態（Ready → mark_refreshing）
        let entry = ready_entry(now, now_wall, RefreshMode::Warm).with_mark_refreshing(now);
        let repo_id = RepoId::new("repo".to_owned());
        let mut entries = std::collections::HashMap::new();
        entries.insert(repo_id.clone(), entry);
        let store = CacheStore::new().with_entries(entries);
        let policy = test_policy();

        let later = now + Duration::from_secs(100);
        let (_new_store, effects) =
            on_query(&store, query_event(later, now_wall), &repo_id, &policy);

        assert_eq!(effects, vec![Effect::EmitOutput("hello".to_owned())]);
    }

    #[test]
    fn on_query_resets_cold_count_when_was_cold() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let entry = ready_entry(now, now_wall, RefreshMode::Warm).with_increment_cold_count();
        assert_eq!(entry.cold_refresh_count(), 1);
        let repo_id = RepoId::new("repo".to_owned());
        let mut entries = std::collections::HashMap::new();
        entries.insert(repo_id.clone(), entry);
        let store = CacheStore::new().with_entries(entries);
        let policy = test_policy();

        // warm_to_cold_secs 超過 = cold
        let later = now + Duration::from_secs(policy.warm_to_cold_secs + 100);
        let (new_store, _effects) =
            on_query(&store, query_event(later, now_wall), &repo_id, &policy);

        let new_entry = new_store.entries().get(&repo_id).expect("entry present");
        assert_eq!(new_entry.cold_refresh_count(), 0);
    }

    #[test]
    fn on_query_terminal_resets_to_warm_when_stale_needs_refresh() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let entry = ready_entry(now, now_wall, RefreshMode::Terminal);
        let repo_id = RepoId::new("repo".to_owned());
        let mut entries = std::collections::HashMap::new();
        entries.insert(repo_id.clone(), entry);
        let store = CacheStore::new().with_entries(entries);
        let policy = test_policy();

        // effective_ttl(Terminal, _) = warm_refresh_secs = 180、それを超える経過
        let later = now + Duration::from_secs(policy.warm_refresh_secs + 10);
        let (new_store, _effects) =
            on_query(&store, query_event(later, now_wall), &repo_id, &policy);

        let new_entry = new_store.entries().get(&repo_id).expect("entry present");
        assert_eq!(new_entry.refresh_mode(), RefreshMode::Warm);
        assert_eq!(new_entry.fetch_state(), FetchState::Refreshing);
    }

    #[test]
    fn on_refresh_completed_updates_state() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let entry = CacheEntryState::new_loading(
            PathBuf::from("/repo/main"),
            "main".to_owned(),
            10,
            now,
            now_wall,
        );
        let repo_id = RepoId::new("repo".to_owned());
        let mut entries = std::collections::HashMap::new();
        entries.insert(repo_id.clone(), entry);
        let store = CacheStore::new().with_entries(entries);

        let later = now + Duration::from_secs(5);
        let event = RefreshCompletedEvent {
            output: "updated".to_owned(),
            pr_outputs: Vec::new(),
            refresh_mode: RefreshMode::Hot,
            now: later,
            now_wall,
        };
        let (new_store, effects) = on_refresh_completed(&store, &repo_id, event);

        assert!(effects.is_empty());
        let new_entry = new_store.entries().get(&repo_id).expect("entry present");
        assert_eq!(new_entry.fetch_state(), FetchState::Ready);
        assert_eq!(new_entry.output(), "updated");
        assert_eq!(new_entry.refresh_mode(), RefreshMode::Hot);
        assert_eq!(new_entry.fetched_at(), later);
    }

    #[test]
    fn on_refresh_completed_ignores_unknown_repo_id() {
        let store = CacheStore::new();
        let repo_id = RepoId::new("unknown".to_owned());
        let event = RefreshCompletedEvent {
            output: "x".to_owned(),
            pr_outputs: Vec::new(),
            refresh_mode: RefreshMode::Warm,
            now: Instant::now(),
            now_wall: SystemTime::now(),
        };
        let (new_store, effects) = on_refresh_completed(&store, &repo_id, event);
        assert!(effects.is_empty());
        assert!(new_store.entries().is_empty());
    }

    // ── on_scheduler_tick ────────────────────────────────────────

    fn tick_input<'a>(
        policy: &'a RefreshPolicy,
        now: Instant,
        now_wall: SystemTime,
    ) -> SchedulerTickInput<'a> {
        SchedulerTickInput {
            now,
            now_wall,
            policy,
            stale_ttl: 10,
            refresh_lock_timeout_secs: 120,
            entry_max_age_secs: 60,
        }
    }

    fn put(store: CacheStore, repo_id: &RepoId, entry: CacheEntryState) -> CacheStore {
        let mut entries = store.entries().clone();
        entries.insert(repo_id.clone(), entry);
        store.with_entries(entries)
    }

    #[test]
    fn on_scheduler_tick_skips_when_backed_off() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let policy = test_policy();
        let repo_id = RepoId::new("repo".to_owned());
        let entry = ready_entry(now, now_wall, RefreshMode::Warm);
        let store = put(CacheStore::new(), &repo_id, entry)
            .with_backoff_until(Some(now + Duration::from_secs(60)));

        let (new_store, effects) = on_scheduler_tick(&store, tick_input(&policy, now, now_wall));

        assert!(effects.is_empty());
        // entries は不変
        assert_eq!(new_store.entries().len(), 1);
        // backoff も維持
        assert!(new_store.backoff_until().is_some());
    }

    #[test]
    fn on_scheduler_tick_clears_expired_backoff() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let policy = test_policy();
        let past = now.checked_sub(Duration::from_secs(10)).expect("past");
        let store = CacheStore::new().with_backoff_until(Some(past));

        let (new_store, _effects) = on_scheduler_tick(&store, tick_input(&policy, now, now_wall));

        assert_eq!(new_store.backoff_until(), None);
    }

    #[test]
    fn on_scheduler_tick_removes_expired_entries() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let policy = test_policy();
        let repo_id = RepoId::new("repo".to_owned());

        // last_queried_at が entry_max_age_secs より過去
        let very_past = now
            .checked_sub(Duration::from_secs(120))
            .expect("very past");
        let mut entry = ready_entry(very_past, now_wall, RefreshMode::Warm);
        entry = entry.with_record_query(very_past);
        let store = put(CacheStore::new(), &repo_id, entry);

        let (new_store, effects) = on_scheduler_tick(&store, tick_input(&policy, now, now_wall));

        assert!(new_store.entries().is_empty());
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::RecordExpired { .. })),
            "RecordExpired effect should be emitted: {effects:?}"
        );
    }

    #[test]
    fn on_scheduler_tick_skips_inactive_terminal() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let policy = test_policy();
        let repo_id = RepoId::new("repo".to_owned());
        let entry = ready_entry(now, now_wall, RefreshMode::Terminal);
        let store = put(CacheStore::new(), &repo_id, entry);

        let (new_store, effects) = on_scheduler_tick(&store, tick_input(&policy, now, now_wall));

        // Terminal は active=true だが effective_refresh_interval_secs が MAX
        // のためタイミング条件でスキップされる。SpawnRefresh は発行されない。
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::SpawnRefresh { .. })),
            "Terminal entry should not spawn refresh: {effects:?}"
        );
        assert_eq!(new_store.entries().len(), 1);
    }

    #[test]
    fn on_scheduler_tick_marks_and_spawns_when_interval_passed() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let policy = test_policy();
        let repo_id = RepoId::new("repo".to_owned());
        // fetched_at を warm_refresh_secs + 余裕分過去にして interval 超過にする
        let past_fetched = now
            .checked_sub(Duration::from_secs(policy.warm_refresh_secs + 60))
            .expect("past");
        let mut entry = CacheEntryState::new_loading(
            PathBuf::from("/repo/main"),
            "main".to_owned(),
            10,
            past_fetched,
            now_wall,
        );
        entry = entry.with_refresh_completed(
            "hello".to_owned(),
            Vec::new(),
            RefreshMode::Warm,
            past_fetched,
            now_wall,
        );
        // last_queried_at を recent にして cold 圏外にする
        entry = entry.with_record_query(now);
        let store = put(CacheStore::new(), &repo_id, entry);

        let (new_store, effects) = on_scheduler_tick(&store, tick_input(&policy, now, now_wall));

        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SpawnRefresh { .. })),
            "interval passed should emit SpawnRefresh: {effects:?}"
        );
        let new_entry = new_store.entries().get(&repo_id).expect("entry present");
        assert_eq!(new_entry.fetch_state(), FetchState::Refreshing);
    }

    #[test]
    fn on_scheduler_tick_clears_refresh_lock_when_timeout() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let policy = test_policy();
        let repo_id = RepoId::new("repo".to_owned());
        // Refreshing + refresh_started_at が timeout 以上過去。
        // last_queried_at は recent にして expired 削除を避ける。
        let past = now.checked_sub(Duration::from_secs(200)).expect("past");
        let mut entry = ready_entry(past, now_wall, RefreshMode::Warm);
        entry = entry.with_record_query(now);
        entry = entry.with_mark_refreshing(past);
        assert_eq!(entry.fetch_state(), FetchState::Refreshing);
        let store = put(CacheStore::new(), &repo_id, entry);

        let (new_store, _effects) = on_scheduler_tick(&store, tick_input(&policy, now, now_wall));

        let new_entry = new_store.entries().get(&repo_id).expect("entry present");
        // lock 解除後、interval 超過なら mark_refreshing し直される。
        // 少なくとも refresh_started_at は now に更新されているか、None にクリア済み。
        assert!(
            new_entry.refresh_started_at().is_none() || new_entry.refresh_started_at() == Some(now)
        );
    }

    // ── total_refresh_cost ───────────────────────────────────────

    fn entry_with_pr_count(pr_count: usize, mode: RefreshMode) -> CacheEntryState {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let mut e = CacheEntryState::new_loading(
            PathBuf::from("/tmp"),
            "main".to_owned(),
            5,
            now,
            now_wall,
        );
        let prs: Vec<crate::shared::protocol::PrOutput> = (0..pr_count)
            .map(|i| crate::shared::protocol::PrOutput {
                pr_id: i as u64,
                output: String::new(),
            })
            .collect();
        e = e.with_refresh_completed("output".to_owned(), prs, mode, now, now_wall);
        e
    }

    #[test]
    fn total_refresh_cost_excludes_terminal() {
        let entries = vec![
            entry_with_pr_count(2, RefreshMode::Warm),
            entry_with_pr_count(0, RefreshMode::Warm),
            entry_with_pr_count(3, RefreshMode::Terminal),
        ];
        assert_eq!(super::total_refresh_cost(&entries), 6);
    }

    #[test]
    fn total_refresh_cost_excludes_loading() {
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let e = CacheEntryState::new_loading(
            PathBuf::from("/tmp"),
            "main".to_owned(),
            5,
            now,
            now_wall,
        );
        assert!(!e.is_active());
        let entries = vec![e];
        assert_eq!(super::total_refresh_cost(&entries), 0);
    }

    // ── on_rate_limit_observed ───────────────────────────────────

    fn make_snapshot(remaining: u32, limit: u32, secs_until_reset: u64) -> RateLimitSnapshot {
        RateLimitSnapshot {
            core_remaining: remaining,
            core_limit: limit,
            graphql_remaining: remaining,
            graphql_limit: limit,
            reset_at: SystemTime::now() + Duration::from_secs(secs_until_reset),
            fetched_at: Instant::now(),
        }
    }

    #[test]
    fn on_rate_limit_observed_updates_snapshot() {
        let store = CacheStore::new();
        // 残量フル (50%) → backoff には入らない
        let snapshot = make_snapshot(5_000, 10_000, 1_800);
        let event = RateLimitObservedEvent {
            snapshot,
            now: Instant::now(),
            now_wall: SystemTime::now(),
        };
        let (new_store, effects) = on_rate_limit_observed(&store, event);

        assert!(effects.is_empty(), "no backoff effect expected");
        assert!(new_store.latest_rate_limit().is_some());
        assert_eq!(new_store.backoff_until(), None);
    }

    #[test]
    fn on_rate_limit_observed_enters_backoff_when_exhausted() {
        let store = CacheStore::new();
        // 完全枯渇
        let snapshot = make_snapshot(0, 10_000, 1_800);
        let event = RateLimitObservedEvent {
            snapshot,
            now: Instant::now(),
            now_wall: SystemTime::now(),
        };
        let (new_store, effects) = on_rate_limit_observed(&store, event);

        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::EnterBackoff { .. })),
            "EnterBackoff effect expected: {effects:?}"
        );
        assert!(new_store.backoff_until().is_some());
    }

    #[test]
    fn on_rate_limit_observed_enters_backoff_when_below_threshold() {
        let store = CacheStore::new();
        // 残量 4% (= 400 bp < 500 bp の閾値)
        let snapshot = make_snapshot(400, 10_000, 1_800);
        let event = RateLimitObservedEvent {
            snapshot,
            now: Instant::now(),
            now_wall: SystemTime::now(),
        };
        let (new_store, effects) = on_rate_limit_observed(&store, event);

        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::EnterBackoff { .. })),
            "EnterBackoff expected when below threshold: {effects:?}"
        );
        assert!(new_store.backoff_until().is_some());
    }
}
