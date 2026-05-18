//! `CacheStore` の純粋遷移関数。
//!
//! daemon の各 edge (`request_handler` / `background_refresh` /
//! `daemon_server::update_state_from_snapshot`) は、ロック取得後に
//! 本モジュールの純粋関数を呼んで `(CacheStore, Vec<Effect>)` を得る。
//! 戻り値の `CacheStore` を `Mutex` 配下の値に書き戻し、`Vec<Effect>` を
//! drain して副作用（リフレッシュ起動、ソケット書き込み、ログ）を実行する。

use std::time::Instant;

use super::effect::Effect;
use super::entry::CacheEntryState;
use super::event::{QueryEvent, RefreshCompletedEvent};
use super::repo_id::RepoId;
use super::store::CacheStore;
use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;
use crate::shared::refresh_mode::RefreshMode;

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
// crate-private 述語ヘルパ（旧 getter の `now` 引数化版）
// ───────────────────────────────────────────────────────────────

pub(super) fn is_fresh(s: &CacheEntryState, ttl: u64, now: Instant) -> bool {
    elapsed_secs(s.fetched_at(), now) <= ttl
}

pub(super) fn is_cold_or_never_queried(
    s: &CacheEntryState,
    warm_to_cold_secs: u64,
    now: Instant,
) -> bool {
    s.last_queried_at()
        .is_none_or(|t| elapsed_secs(t, now) >= warm_to_cold_secs)
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
}
