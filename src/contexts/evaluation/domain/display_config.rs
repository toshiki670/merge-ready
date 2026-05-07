use serde::Serialize;

mod defaults;
mod render;

pub use render::{render_error_token, render_token};

#[derive(Serialize)]
pub struct DisplayConfig {
    pub merge_ready: TokenConfig,
    pub no_pull_request: TokenConfig,
    pub conflict: TokenConfig,
    pub update_branch: TokenConfig,
    pub sync_unknown: TokenConfig,
    pub ci_fail: TokenConfig,
    pub ci_action: TokenConfig,
    pub ci_pending: TokenConfig,
    pub changes_requested: TokenConfig,
    pub review_required: TokenConfig,
    pub draft: TokenConfig,
    pub status_calculating: TokenConfig,
    pub blocked_unknown: TokenConfig,
    pub error: ErrorConfig,
}

pub trait DisplayConfigRepository {
    fn load(&self) -> DisplayConfig;
}

#[derive(Serialize)]
pub struct TokenConfig {
    pub symbol: String,
    pub label: String,
    pub format: String,
}

#[derive(Serialize)]
pub struct ErrorConfig {
    pub symbol: String,
    pub format: String,
}
