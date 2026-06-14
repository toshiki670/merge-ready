use std::time::{Instant, SystemTime};

/// `bottleneck_ratio` の basis points スケール（10000 == 1.0）。
pub const RATIO_SCALE_BP: u64 = 10_000;

/// `remaining / limit` を basis points（0..=`RATIO_SCALE_BP`）で返す。
/// `limit == 0` のリソースは比率不定として `RATIO_SCALE_BP`（=1.0）を採用する。
fn ratio_bp(remaining: u32, limit: u32) -> u64 {
    if limit == 0 {
        return RATIO_SCALE_BP;
    }
    (u64::from(remaining).saturating_mul(RATIO_SCALE_BP)) / u64::from(limit)
}

/// `gh api rate_limit` から得た残量スナップショット。
///
/// `core` と `graphql` の両方を保持し、両者のうち消費比率の高い側を
/// ボトルネックとみなして判定する。`fetched_at` は 60 秒キャッシュの
/// 鮮度判定に、`reset_at` はリセット時刻までの残時間計算に使う。
#[derive(Debug, Clone, Copy)]
pub struct RateLimitSnapshot {
    pub core_remaining: u32,
    pub core_limit: u32,
    pub graphql_remaining: u32,
    pub graphql_limit: u32,
    pub reset_at: SystemTime,
    pub fetched_at: Instant,
}

impl RateLimitSnapshot {
    /// `now` から `reset_at` までの秒数。既に過ぎていれば 0。
    #[must_use]
    pub fn secs_until_reset(&self, now: SystemTime) -> u64 {
        self.reset_at.duration_since(now).map_or(0, |d| d.as_secs())
    }

    /// `core` または `graphql` の `remaining` が 0 なら枯渇とみなす。
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.core_remaining == 0 || self.graphql_remaining == 0
    }

    /// `min(core_ratio, graphql_ratio)` を basis points（0..=`RATIO_SCALE_BP`）で返す。
    /// 消費比率の高い側（残量比率の小さい側）をボトルネックとみなす。
    /// `limit == 0` のリソースは比率不定として採用優先順位から外れる。両方 0 なら
    /// `RATIO_SCALE_BP`。
    #[must_use]
    pub fn bottleneck_ratio_bp(&self) -> u64 {
        ratio_bp(self.core_remaining, self.core_limit)
            .min(ratio_bp(self.graphql_remaining, self.graphql_limit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn snapshot(
        core_remaining: u32,
        core_limit: u32,
        graphql_remaining: u32,
        graphql_limit: u32,
        reset_at: SystemTime,
    ) -> RateLimitSnapshot {
        RateLimitSnapshot {
            core_remaining,
            core_limit,
            graphql_remaining,
            graphql_limit,
            reset_at,
            fetched_at: Instant::now(),
        }
    }

    #[test]
    fn secs_until_reset_returns_remaining_time() {
        let now = SystemTime::now();
        let s = snapshot(0, 0, 0, 0, now + Duration::from_mins(2));
        assert_eq!(s.secs_until_reset(now), 120);
    }

    #[test]
    fn secs_until_reset_returns_zero_when_already_past() {
        let now = SystemTime::now();
        let s = snapshot(0, 0, 0, 0, now - Duration::from_secs(10));
        assert_eq!(s.secs_until_reset(now), 0);
    }

    #[test]
    fn is_exhausted_when_core_remaining_zero() {
        let s = snapshot(0, 5000, 5000, 5000, SystemTime::now());
        assert!(s.is_exhausted());
    }

    #[test]
    fn is_exhausted_when_graphql_remaining_zero() {
        let s = snapshot(5000, 5000, 0, 5000, SystemTime::now());
        assert!(s.is_exhausted());
    }

    #[test]
    fn is_exhausted_false_when_both_have_remaining() {
        let s = snapshot(1, 5000, 1, 5000, SystemTime::now());
        assert!(!s.is_exhausted());
    }

    #[test]
    fn bottleneck_ratio_bp_picks_smaller_side() {
        // core 50% (5000 bp) と graphql 10% (1000 bp) → 小さい方 1000 bp。
        let s = snapshot(5000, 10_000, 1000, 10_000, SystemTime::now());
        assert_eq!(s.bottleneck_ratio_bp(), 1000);
    }

    #[test]
    fn bottleneck_ratio_bp_full_when_remaining_is_full() {
        let s = snapshot(10_000, 10_000, 10_000, 10_000, SystemTime::now());
        assert_eq!(s.bottleneck_ratio_bp(), RATIO_SCALE_BP);
    }

    #[test]
    fn bottleneck_ratio_bp_treats_zero_limit_as_full() {
        // limit == 0 は比率不定 → RATIO_SCALE_BP として採用優先順位から外れ、
        // もう一方（graphql 25% = 2500 bp）が採用される。
        let s = snapshot(0, 0, 2500, 10_000, SystemTime::now());
        assert_eq!(s.bottleneck_ratio_bp(), 2500);
    }
}
