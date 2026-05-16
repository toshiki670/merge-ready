use std::time::{Instant, SystemTime};

/// `gh api rate_limit` から得た残量スナップショット。
///
/// `core` と `graphql` の両方を保持し、両者のうち消費比率の高い側を
/// ボトルネックとみなして判定する。`fetched_at` は 60 秒キャッシュの
/// 鮮度判定に、`reset_at` はリセット時刻までの残時間計算に使う。
// 後続コミットで RefreshPolicy 等から実利用するまでの間、dead_code を抑制する。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct RateLimitSnapshot {
    pub core_remaining: u32,
    pub core_limit: u32,
    pub graphql_remaining: u32,
    pub graphql_limit: u32,
    pub reset_at: SystemTime,
    pub fetched_at: Instant,
}

#[allow(dead_code)]
impl RateLimitSnapshot {
    /// `remaining / limit` 比率のうち低い方（= 最も切迫したリソース）を返す。
    ///
    /// limit が 0 のリソースは比率不定として 1.0 とみなす（無視する）。
    /// 両方とも limit=0 の場合は 1.0 を返す。
    #[must_use]
    pub fn bottleneck_ratio(&self) -> f64 {
        let core = resource_ratio(self.core_remaining, self.core_limit);
        let graphql = resource_ratio(self.graphql_remaining, self.graphql_limit);
        core.min(graphql)
    }

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
}

#[allow(dead_code)]
fn resource_ratio(remaining: u32, limit: u32) -> f64 {
    if limit == 0 {
        1.0
    } else {
        f64::from(remaining) / f64::from(limit)
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
    fn bottleneck_ratio_picks_lower_resource() {
        let s = snapshot(2500, 5000, 500, 5000, SystemTime::now());
        // core = 0.5, graphql = 0.1 → 0.1 を採用
        assert!((s.bottleneck_ratio() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn bottleneck_ratio_full_when_both_full() {
        let s = snapshot(5000, 5000, 5000, 5000, SystemTime::now());
        assert!((s.bottleneck_ratio() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bottleneck_ratio_zero_when_either_remaining_is_zero() {
        let s = snapshot(0, 5000, 5000, 5000, SystemTime::now());
        assert!((s.bottleneck_ratio() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn bottleneck_ratio_treats_zero_limit_as_full() {
        // limit=0 のリソースは比率不定として 1.0 → 採用されない
        let s = snapshot(0, 0, 100, 1000, SystemTime::now());
        assert!((s.bottleneck_ratio() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn bottleneck_ratio_both_zero_limit_returns_full() {
        let s = snapshot(0, 0, 0, 0, SystemTime::now());
        assert!((s.bottleneck_ratio() - 1.0).abs() < 1e-9);
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
}
