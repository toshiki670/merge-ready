use serde::Deserialize;
use std::path::PathBuf;

use crate::contexts::evaluation::domain::display_config::{
    DisplayConfig, DisplayConfigRepository, ErrorConfig, TokenConfig,
};

pub struct TomlConfigRepository;

#[allow(clippy::unused_self)]
impl DisplayConfigRepository for TomlConfigRepository {
    fn load(&self) -> DisplayConfig {
        let Some(path) = config_path() else {
            return DisplayConfig::default();
        };
        let Ok(content) = std::fs::read_to_string(path) else {
            return DisplayConfig::default();
        };
        let raw: RawDisplayConfig = toml::from_str(&content).unwrap_or_default();
        merge_with_defaults(raw)
    }
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

// XDG_CONFIG_HOME が設定されていればそちらを優先し、なければ $HOME/.config にフォールバックする。
pub(crate) fn config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("merge-ready.toml"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config").join("merge-ready.toml"))
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
