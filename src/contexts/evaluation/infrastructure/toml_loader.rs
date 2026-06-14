use serde::Deserialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::contexts::evaluation::domain::display_config::{
    CompiledDisplayConfig, DisplayConfig, ErrorConfig, TokenConfig,
};

struct CachedConfig {
    fingerprint: Option<u64>,
    config: Arc<CompiledDisplayConfig>,
}

/// プロセス内に保持する前計算済み設定キャッシュ。daemon は単一プロセスで設定ファイルも
/// 1 つなので、プロセスグローバルなキャッシュで足りる。
static CONFIG_CACHE: OnceLock<Mutex<Option<CachedConfig>>> = OnceLock::new();

/// 設定ファイルを非同期で読み込み、前計算済みの [`CompiledDisplayConfig`] を返す。
///
/// ファイル内容の xxHash 指紋をメモリに保持し、指紋が前回と一致する場合は前計算済みの
/// 結果（`Arc`）をそのまま返す。ファイルが変更されたとき（または初回）だけ TOML パース・
/// デフォルトマージ・format/style の前計算をやり直す。これにより daemon リフレッシュ経路で
/// 不変な設定を毎回再パースする無駄をなくす。
pub async fn load_compiled_display_config() -> Arc<CompiledDisplayConfig> {
    let content = read_config_bytes().await;
    let fp = fingerprint(content.as_deref());

    let cache = CONFIG_CACHE.get_or_init(|| Mutex::new(None));
    // ファイル読み込み（await）はロック取得前に完了している。ロック保持中は await しない
    // ので、std の Mutex で安全に扱える。
    if let Some(hit) = {
        let guard = cache.lock().expect("config cache mutex poisoned");
        guard
            .as_ref()
            .filter(|cached| cached.fingerprint == fp)
            .map(|cached| Arc::clone(&cached.config))
    } {
        return hit;
    }

    let display = content
        .as_deref()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .map_or_else(DisplayConfig::default, parse_display_config);
    let compiled = Arc::new(display.compile());

    *cache.lock().expect("config cache mutex poisoned") = Some(CachedConfig {
        fingerprint: fp,
        config: Arc::clone(&compiled),
    });
    compiled
}

/// 設定ファイルの内容を非同期で読み込む。ファイル不在・読取不可なら `None`。
async fn read_config_bytes() -> Option<Vec<u8>> {
    tokio::fs::read(config_path()?).await.ok()
}

/// 設定ファイル内容の xxHash 指紋。内容が無い（ファイル不在等）場合は `None`。
/// 変更検知が目的でセキュリティ用途ではないため、高速な非暗号学的ハッシュを使う。
fn fingerprint(content: Option<&[u8]>) -> Option<u64> {
    content.map(twox_hash::XxHash3_64::oneshot)
}

/// TOML 文字列を `DisplayConfig` にパースする。壊れた TOML は default で埋める。
fn parse_display_config(content: &str) -> DisplayConfig {
    let raw: RawDisplayConfig = toml::from_str(content).unwrap_or_default();
    merge_with_defaults(raw)
}

fn merge_with_defaults(raw: RawDisplayConfig) -> DisplayConfig {
    let defaults = DisplayConfig::default();
    DisplayConfig {
        merge_ready: merge_token(raw.merge_ready, defaults.merge_ready),
        no_pull_request: merge_token(raw.no_pull_request, defaults.no_pull_request),
        conflict: merge_token(raw.conflict, defaults.conflict),
        update_branch: merge_token(raw.update_branch, defaults.update_branch),
        sync_unknown: merge_token(raw.sync_unknown, defaults.sync_unknown),
        ci_fail: merge_token(raw.ci_fail, defaults.ci_fail),
        ci_action: merge_token(raw.ci_action, defaults.ci_action),
        ci_pending: merge_token(raw.ci_pending, defaults.ci_pending),
        changes_requested: merge_token(raw.changes_requested, defaults.changes_requested),
        review_required: merge_token(raw.review_required, defaults.review_required),
        draft: merge_token(raw.draft, defaults.draft),
        status_calculating: merge_token(raw.status_calculating, defaults.status_calculating),
        blocked_unknown: merge_token(raw.blocked_unknown, defaults.blocked_unknown),
        error: merge_error(raw.error, defaults.error),
    }
}

fn merge_token(raw: Option<RawTokenConfig>, default: TokenConfig) -> TokenConfig {
    let Some(raw) = raw else {
        return default;
    };
    TokenConfig {
        symbol: raw.symbol.unwrap_or(default.symbol),
        label: raw.label.unwrap_or(default.label),
        format: raw.format.unwrap_or(default.format),
    }
}

fn merge_error(raw: Option<RawErrorConfig>, default: ErrorConfig) -> ErrorConfig {
    let Some(raw) = raw else {
        return default;
    };
    ErrorConfig {
        symbol: raw.symbol.unwrap_or(default.symbol),
        format: raw.format.unwrap_or(default.format),
    }
}

// XDG_CONFIG_HOME が有効な絶対パスならそちらを優先し、無効（未設定・空・相対パス）なら
// $HOME/.config にフォールバックする。
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    let base = match super::xdg::base_dir("XDG_CONFIG_HOME") {
        Some(dir) => dir,
        None => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("merge-ready.toml"))
}

#[derive(Deserialize, Default)]
struct RawDisplayConfig {
    merge_ready: Option<RawTokenConfig>,
    no_pull_request: Option<RawTokenConfig>,
    conflict: Option<RawTokenConfig>,
    update_branch: Option<RawTokenConfig>,
    sync_unknown: Option<RawTokenConfig>,
    ci_fail: Option<RawTokenConfig>,
    ci_action: Option<RawTokenConfig>,
    ci_pending: Option<RawTokenConfig>,
    changes_requested: Option<RawTokenConfig>,
    review_required: Option<RawTokenConfig>,
    draft: Option<RawTokenConfig>,
    status_calculating: Option<RawTokenConfig>,
    blocked_unknown: Option<RawTokenConfig>,
    error: Option<RawErrorConfig>,
}

#[derive(Deserialize)]
struct RawTokenConfig {
    symbol: Option<String>,
    label: Option<String>,
    format: Option<String>,
}

#[derive(Deserialize)]
struct RawErrorConfig {
    symbol: Option<String>,
    format: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_none_for_absent_content() {
        assert_eq!(fingerprint(None), None);
    }

    #[test]
    fn fingerprint_is_stable_for_same_content() {
        assert_eq!(fingerprint(Some(b"abc")), fingerprint(Some(b"abc")));
    }

    #[test]
    fn fingerprint_differs_for_changed_content() {
        assert_ne!(fingerprint(Some(b"abc")), fingerprint(Some(b"abd")));
    }

    #[test]
    fn fingerprint_distinguishes_empty_file_from_absent() {
        // 空ファイル（存在する）と不在（None）は別物として扱われる。
        assert!(fingerprint(Some(b"")).is_some());
    }

    #[test]
    fn parse_display_config_applies_overrides() {
        let parsed = parse_display_config("[merge_ready]\nsymbol = \"★\"");
        assert_eq!(parsed.merge_ready.symbol, "★");
    }

    #[test]
    fn parse_display_config_invalid_toml_uses_defaults() {
        let parsed = parse_display_config("][[[ not valid toml");
        assert_eq!(
            parsed.merge_ready.symbol,
            DisplayConfig::default().merge_ready.symbol
        );
    }
}
