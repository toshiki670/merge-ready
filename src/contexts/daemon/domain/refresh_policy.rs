use std::time::{Instant, SystemTime};

use crate::contexts::daemon::domain::cache::{CacheEntryState, has_recent_query, is_cold};
use crate::contexts::daemon::domain::rate_limit_snapshot::{RATIO_SCALE_BP, RateLimitSnapshot};
use crate::shared::refresh_mode::RefreshMode;

/// 安全マージン（残量から 5% を予備として控除）。`budget * SAFETY_NUM / SAFETY_DEN`。
const SAFETY_NUM: u64 = 95;
const SAFETY_DEN: u64 = 100;

/// 予算ベース項の計算で `secs_until_reset` をキャップする上限秒数。
/// GitHub の主要レート制限は 1 時間ウィンドウなので、未来極端な `reset_at`
/// （テスト `fixture` 等）が来たときに `budget_term` が膨張するのを防ぐ。
const RESET_WINDOW_CAP_SECS: u64 = 3_600;

/// エントリの現在状態から導かれる、スケーリング上の実効モード。
/// Warm + 最近 Query あり は Hot 相当、Warm + Cold 圏 は Cold 相当として扱う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectiveMode {
    Hot,
    Warm,
    Cold,
}

/// 実効モードごとのスケーリング係数（整数演算のため 10 倍したスケール）。
/// - `alpha_x10`: ratio ベース 2 次関数 `base * (1 + alpha * (1 - ratio)^2)` の係数 × 10
/// - `weight_x10`: 予算ベース項に乗じる重み × 10。Hot ほど小さく（≒ より頻繁）、Cold ほど大きい
struct ScalingParams {
    alpha_x10: u64,
    weight_x10: u64,
}

const SCALE_X10: u64 = 10;

fn scaling_params(mode: EffectiveMode) -> ScalingParams {
    match mode {
        EffectiveMode::Hot => ScalingParams {
            alpha_x10: 30,
            weight_x10: 5,
        },
        EffectiveMode::Warm => ScalingParams {
            alpha_x10: 70,
            weight_x10: 10,
        },
        EffectiveMode::Cold => ScalingParams {
            alpha_x10: 150,
            weight_x10: 20,
        },
    }
}

/// Hot/Warm/Cold 各モードのリフレッシュ間隔と TTL ルールを保持するドメインサービス。
#[derive(Debug, Clone, Copy)]
pub struct RefreshPolicy {
    /// "最近 Query あり" とみなす経過秒数（Hot/Warm 共通）
    pub hot_recent_query_secs: u64,
    /// Hot または Warm + 最近 Query あり の場合のリフレッシュ間隔
    pub hot_with_query_secs: u64,
    /// Hot（Query なし）の場合のリフレッシュ間隔
    pub hot_without_query_secs: u64,
    /// Warm モードの標準リフレッシュ間隔
    pub warm_refresh_secs: u64,
    /// Warm から Cold へ移行するまでの Query 無し経過秒数
    pub warm_to_cold_secs: u64,
    /// Cold 初期（累計リフレッシュ `cold_early_limit` 回まで）の間隔
    pub cold_early_secs: u64,
    /// Cold 後期（`cold_early_limit` 回超）の間隔
    pub cold_late_secs: u64,
    /// Cold 初期から後期へ切り替わる累計リフレッシュ回数
    pub cold_early_limit: u32,
}

impl RefreshPolicy {
    /// エントリの現在の状態からリフレッシュ間隔（秒）を返す。
    pub fn effective_refresh_interval_secs(&self, entry: &CacheEntryState, now: Instant) -> u64 {
        match entry.refresh_mode() {
            RefreshMode::Terminal => u64::MAX,
            RefreshMode::Hot => {
                if has_recent_query(entry, self.hot_recent_query_secs, now) {
                    self.hot_with_query_secs
                } else {
                    self.hot_without_query_secs
                }
            }
            RefreshMode::Warm => {
                if has_recent_query(entry, self.hot_recent_query_secs, now) {
                    self.hot_with_query_secs
                } else if is_cold(entry, self.warm_to_cold_secs, now) {
                    self.cold_interval_secs(entry.cold_refresh_count())
                } else {
                    self.warm_refresh_secs
                }
            }
        }
    }

    /// Terminal エントリは `warm_refresh_secs` を TTL として返す（PR 再オープン検知のため）。
    pub fn effective_ttl(&self, entry: &CacheEntryState, base_ttl: u64) -> u64 {
        if entry.refresh_mode() == RefreshMode::Terminal {
            self.warm_refresh_secs
        } else {
            base_ttl
        }
    }

    fn cold_interval_secs(&self, count: u32) -> u64 {
        if count < self.cold_early_limit {
            self.cold_early_secs
        } else {
            self.cold_late_secs
        }
    }

    /// `effective_refresh_interval_secs` に `rate_limit` 残量による動的スケーリングを
    /// 加味した間隔（秒）を返す。
    ///
    /// 下限は常に既存の `effective_refresh_interval_secs(entry)` で、上限なし。
    /// `snapshot` が `None`（`rate_limit` 取得 OFF / 失敗中）なら従来挙動と同値。
    /// Terminal エントリは `u64::MAX`（既存挙動）。
    ///
    /// 計算は 2 つの項のうち大きい方を採用する（ハイブリッド下限）:
    /// - **ratio 項**: `base * (1 + α * (1 - ratio)^2)`
    /// - **予算項**: `total_cost_per_cycle * secs_until_reset / safety_budget * weight`
    ///
    /// `total_cost_per_cycle` は全 active エントリの「1 リフレッシュあたりの API
    /// コール数」総和（呼び出し側が `pr_count + 1` の総和を渡す）。
    ///
    /// `is_exhausted()` のとき、結果は最低でも `cold_late_secs` 以上に保証する。
    ///
    /// 計算は浮動小数を使わず、basis points（10000=1.0）と 10 倍スケールの整数
    /// 演算のみで完結させる（プロジェクトの clippy pedantic / clippy allow 禁止
    /// ルールへの適合のため）。
    #[must_use]
    pub fn effective_refresh_interval_secs_scaled(
        &self,
        entry: &CacheEntryState,
        snapshot: Option<&RateLimitSnapshot>,
        total_cost_per_cycle: u64,
        now: Instant,
        now_wall: SystemTime,
    ) -> u64 {
        let base = self.effective_refresh_interval_secs(entry, now);
        if base == u64::MAX {
            return base; // Terminal
        }
        let Some(snapshot) = snapshot else {
            return base;
        };

        let mode = self.effective_mode(entry, now);
        let params = scaling_params(mode);
        let ratio_bp = snapshot.bottleneck_ratio_bp();
        let secs_until_reset = snapshot
            .secs_until_reset(now_wall)
            .min(RESET_WINDOW_CAP_SECS);

        let ratio_term = compute_ratio_term(base, ratio_bp, params.alpha_x10);
        let budget_term = compute_budget_term(
            total_cost_per_cycle,
            secs_until_reset,
            bottleneck_budget(snapshot),
            params.weight_x10,
            ratio_bp,
        );

        let scaled = base.max(ratio_term).max(budget_term);

        if snapshot.is_exhausted() {
            scaled.max(self.cold_late_secs)
        } else {
            scaled
        }
    }

    fn effective_mode(&self, entry: &CacheEntryState, now: Instant) -> EffectiveMode {
        match entry.refresh_mode() {
            // 呼び出し元は Terminal 前に return しているはずだが、フォールバックとして Cold 扱い
            RefreshMode::Terminal => EffectiveMode::Cold,
            RefreshMode::Hot => EffectiveMode::Hot,
            RefreshMode::Warm => {
                if has_recent_query(entry, self.hot_recent_query_secs, now) {
                    EffectiveMode::Hot
                } else if is_cold(entry, self.warm_to_cold_secs, now) {
                    EffectiveMode::Cold
                } else {
                    EffectiveMode::Warm
                }
            }
        }
    }
}

fn bottleneck_budget(snapshot: &RateLimitSnapshot) -> u32 {
    snapshot.core_remaining.min(snapshot.graphql_remaining)
}

/// `base * (1 + alpha * (1 - ratio)^2)` を整数演算で計算する。
/// `alpha_x10` は α を 10 倍したスケール、`ratio_bp` は ratio を `RATIO_SCALE_BP` 倍したスケール。
fn compute_ratio_term(base: u64, ratio_bp: u64, alpha_x10: u64) -> u64 {
    let one_minus_r = RATIO_SCALE_BP.saturating_sub(ratio_bp);
    // (1 - r)^2 in basis points
    let one_minus_r_sq_bp = one_minus_r.saturating_mul(one_minus_r) / RATIO_SCALE_BP;
    // bonus = base * alpha_x10 * one_minus_r_sq_bp / (SCALE_X10 * RATIO_SCALE_BP)
    let bonus_numer = base
        .saturating_mul(alpha_x10)
        .saturating_mul(one_minus_r_sq_bp);
    let bonus = bonus_numer / (SCALE_X10.saturating_mul(RATIO_SCALE_BP));
    base.saturating_add(bonus)
}

/// 予算ベース項を整数演算で計算する。
///
/// 基本式: `total_cost * secs_until_reset / safety_budget * weight`。
/// これに「残量豊富時はスケーリングを無効化する」ゲートとして `(1 - ratio)` を乗じる。
/// `ratio=1.0` のときゲート=0 で項は 0 になり、`base` が支配的になる（要件:
/// 「下限＝現デフォルト値」を満たす）。`ratio=0.0` で full effect。
///
/// `budget_remaining == 0` のときは `safety_budget = 1` にフォールバック（呼び出し側で
/// `is_exhausted()` 後処理として `cold_late_secs` 下界が追加される）。
fn compute_budget_term(
    total_cost_per_cycle: u64,
    secs_until_reset: u64,
    budget_remaining: u32,
    weight_x10: u64,
    ratio_bp: u64,
) -> u64 {
    let safety = u64::from(budget_remaining).saturating_mul(SAFETY_NUM) / SAFETY_DEN;
    let safety = safety.max(1);
    let depletion_bp = RATIO_SCALE_BP.saturating_sub(ratio_bp);
    // numer = cost * window * weight_x10 * depletion_bp
    let numer = total_cost_per_cycle
        .saturating_mul(secs_until_reset)
        .saturating_mul(weight_x10)
        .saturating_mul(depletion_bp);
    // denom = safety * SCALE_X10 * RATIO_SCALE_BP
    let denom = safety
        .saturating_mul(SCALE_X10)
        .saturating_mul(RATIO_SCALE_BP);
    if denom == 0 {
        return u64::MAX;
    }
    numer / denom
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

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

    /// `mode` のエントリを構築し、`last_queried_at` を `now - ago_secs` にセットする。
    /// テスト時間を `now` で固定するため `now` を引数で受ける。
    fn entry_for_test(
        mode: RefreshMode,
        last_queried_ago_secs: u64,
        cold_refresh_count: u32,
        now: Instant,
    ) -> CacheEntryState {
        use std::time::SystemTime;
        let now_wall = SystemTime::now();
        let past = now
            .checked_sub(Duration::from_secs(last_queried_ago_secs))
            .unwrap_or(now);
        CacheEntryState::builder_for_test(now, now_wall)
            .output("output".to_owned())
            .refresh_mode(mode)
            .last_queried_at(Some(past))
            .cold_refresh_count(cold_refresh_count)
            .build()
    }

    #[test]
    fn terminal_returns_max() {
        let policy = test_policy();
        let now = Instant::now();
        let entry = entry_for_test(RefreshMode::Terminal, 0, 0, now);
        assert_eq!(
            policy.effective_refresh_interval_secs(&entry, now),
            u64::MAX
        );
    }

    #[test]
    fn hot_with_recent_query_uses_hot_with_query_secs() {
        let policy = test_policy();
        let now = Instant::now();
        let entry = entry_for_test(RefreshMode::Hot, 5, 0, now);
        assert_eq!(
            policy.effective_refresh_interval_secs(&entry, now),
            policy.hot_with_query_secs
        );
    }

    #[test]
    fn hot_without_recent_query_uses_hot_without_query_secs() {
        let policy = test_policy();
        let now = Instant::now();
        let entry = entry_for_test(RefreshMode::Hot, policy.hot_recent_query_secs + 5, 0, now);
        assert_eq!(
            policy.effective_refresh_interval_secs(&entry, now),
            policy.hot_without_query_secs
        );
    }

    #[test]
    fn warm_with_recent_query_uses_hot_with_query_secs() {
        let policy = test_policy();
        let now = Instant::now();
        let entry = entry_for_test(RefreshMode::Warm, 5, 0, now);
        assert_eq!(
            policy.effective_refresh_interval_secs(&entry, now),
            policy.hot_with_query_secs
        );
    }

    #[test]
    fn warm_without_recent_query_uses_warm_refresh_secs() {
        let policy = test_policy();
        let now = Instant::now();
        let entry = entry_for_test(RefreshMode::Warm, policy.hot_recent_query_secs + 5, 0, now);
        assert_eq!(
            policy.effective_refresh_interval_secs(&entry, now),
            policy.warm_refresh_secs
        );
    }

    #[test]
    fn warm_in_cold_range_uses_cold_early_when_count_below_limit() {
        let policy = test_policy();
        let now = Instant::now();
        let entry = entry_for_test(RefreshMode::Warm, policy.warm_to_cold_secs + 60, 0, now);
        assert_eq!(
            policy.effective_refresh_interval_secs(&entry, now),
            policy.cold_early_secs
        );
    }

    #[test]
    fn warm_in_cold_range_uses_cold_late_when_count_at_limit() {
        let policy = test_policy();
        let now = Instant::now();
        let entry = entry_for_test(
            RefreshMode::Warm,
            policy.warm_to_cold_secs + 60,
            policy.cold_early_limit,
            now,
        );
        assert_eq!(
            policy.effective_refresh_interval_secs(&entry, now),
            policy.cold_late_secs
        );
    }

    #[test]
    fn effective_ttl_returns_warm_for_terminal() {
        let policy = test_policy();
        let entry = entry_for_test(RefreshMode::Terminal, 0, 0, Instant::now());
        assert_eq!(policy.effective_ttl(&entry, 9999), policy.warm_refresh_secs);
    }

    #[test]
    fn effective_ttl_returns_base_for_non_terminal() {
        let policy = test_policy();
        let entry = entry_for_test(RefreshMode::Warm, 0, 0, Instant::now());
        assert_eq!(policy.effective_ttl(&entry, 42), 42);
    }

    // ── effective_refresh_interval_secs_scaled ────────────────────────────

    /// テスト用ヘルパ。`remaining_bp` は 0..=10000（10000 == 残量フル）。
    fn snapshot_at_bp(remaining_bp: u32, secs_until_reset: u64) -> RateLimitSnapshot {
        let limit: u32 = 10_000;
        let remaining = remaining_bp.min(limit);
        let reset_at = SystemTime::now() + Duration::from_secs(secs_until_reset);
        RateLimitSnapshot {
            core_remaining: remaining,
            core_limit: limit,
            graphql_remaining: remaining,
            graphql_limit: limit,
            core_reset_at: reset_at,
            graphql_reset_at: reset_at,
            fetched_at: Instant::now(),
        }
    }

    #[test]
    fn scaled_returns_base_when_snapshot_is_none() {
        let policy = test_policy();
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let entry = entry_for_test(RefreshMode::Hot, 5, 0, now);
        assert_eq!(
            policy.effective_refresh_interval_secs_scaled(&entry, None, 3, now, now_wall),
            policy.hot_with_query_secs
        );
    }

    #[test]
    fn scaled_returns_max_for_terminal() {
        let policy = test_policy();
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let entry = entry_for_test(RefreshMode::Terminal, 0, 0, now);
        let snap = snapshot_at_bp(5_000, 1800);
        assert_eq!(
            policy.effective_refresh_interval_secs_scaled(&entry, Some(&snap), 3, now, now_wall),
            u64::MAX
        );
    }

    #[test]
    fn scaled_equals_base_when_full_budget() {
        let policy = test_policy();
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let entry = entry_for_test(RefreshMode::Hot, 5, 0, now);
        // ratio=1.0, secs_until_reset=3600, total_cost が小さいので budget_term も小
        let snap = snapshot_at_bp(10_000, 3600);
        let v =
            policy.effective_refresh_interval_secs_scaled(&entry, Some(&snap), 3, now, now_wall);
        assert_eq!(v, policy.hot_with_query_secs);
    }

    #[test]
    fn scaled_hot_lower_than_warm_lower_than_cold_when_budget_tight() {
        let policy = test_policy();
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let snap = snapshot_at_bp(0, 3600);

        let hot = entry_for_test(RefreshMode::Hot, 5, 0, now);
        let warm = entry_for_test(RefreshMode::Warm, policy.hot_recent_query_secs + 5, 0, now);
        let cold = entry_for_test(RefreshMode::Warm, policy.warm_to_cold_secs + 60, 0, now);

        let hot_v =
            policy.effective_refresh_interval_secs_scaled(&hot, Some(&snap), 3, now, now_wall);
        let warm_v =
            policy.effective_refresh_interval_secs_scaled(&warm, Some(&snap), 3, now, now_wall);
        let cold_v =
            policy.effective_refresh_interval_secs_scaled(&cold, Some(&snap), 3, now, now_wall);

        assert!(hot_v <= warm_v, "Hot ({hot_v}) <= Warm ({warm_v})");
        assert!(warm_v <= cold_v, "Warm ({warm_v}) <= Cold ({cold_v})");
        assert!(hot_v < cold_v, "Hot ({hot_v}) < Cold ({cold_v})");
    }

    #[test]
    fn scaled_floors_at_cold_late_secs_when_exhausted() {
        let policy = test_policy();
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let entry = entry_for_test(RefreshMode::Hot, 5, 0, now);
        let snap = snapshot_at_bp(0, 60);
        let v =
            policy.effective_refresh_interval_secs_scaled(&entry, Some(&snap), 3, now, now_wall);
        assert!(
            v >= policy.cold_late_secs,
            "exhausted should floor at cold_late_secs, got {v}"
        );
    }

    #[test]
    fn scaled_never_below_base() {
        let policy = test_policy();
        let now = Instant::now();
        let now_wall = SystemTime::now();
        let snap = snapshot_at_bp(9_900, 0);
        let entry = entry_for_test(RefreshMode::Warm, policy.hot_recent_query_secs + 5, 0, now);
        let v =
            policy.effective_refresh_interval_secs_scaled(&entry, Some(&snap), 3, now, now_wall);
        assert!(
            v >= policy.warm_refresh_secs,
            "scaled ({v}) must be >= base warm interval ({})",
            policy.warm_refresh_secs
        );
    }
}
