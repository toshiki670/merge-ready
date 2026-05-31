use serde::Serialize;

use crate::contexts::evaluation::domain::format_parser::{CompiledSegment, compile_segments};

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

/// [`DisplayConfig`] をロード時に前計算した render 用の値オブジェクト。
///
/// 各トークンの `format` は [`CompiledSegment`] 列へ、`style` は解決済みとして
/// 保持される。render はこのツリーを評価するだけで、`format` の再パースや
/// `StyleSpec::parse` を行わない。
pub struct CompiledDisplayConfig {
    pub merge_ready: CompiledTokenConfig,
    pub no_pull_request: CompiledTokenConfig,
    pub conflict: CompiledTokenConfig,
    pub update_branch: CompiledTokenConfig,
    pub sync_unknown: CompiledTokenConfig,
    pub ci_fail: CompiledTokenConfig,
    pub ci_action: CompiledTokenConfig,
    pub ci_pending: CompiledTokenConfig,
    pub changes_requested: CompiledTokenConfig,
    pub review_required: CompiledTokenConfig,
    pub draft: CompiledTokenConfig,
    pub status_calculating: CompiledTokenConfig,
    pub blocked_unknown: CompiledTokenConfig,
    pub error: CompiledErrorConfig,
}

pub struct CompiledTokenConfig {
    pub symbol: String,
    pub label: String,
    pub segments: Vec<CompiledSegment>,
}

pub struct CompiledErrorConfig {
    pub symbol: String,
    pub segments: Vec<CompiledSegment>,
}

impl DisplayConfig {
    /// 設定ロード時に一度だけ呼び、全トークンの `format` を前計算する。
    #[must_use]
    pub fn compile(&self) -> CompiledDisplayConfig {
        CompiledDisplayConfig {
            merge_ready: self.merge_ready.compile(),
            no_pull_request: self.no_pull_request.compile(),
            conflict: self.conflict.compile(),
            update_branch: self.update_branch.compile(),
            sync_unknown: self.sync_unknown.compile(),
            ci_fail: self.ci_fail.compile(),
            ci_action: self.ci_action.compile(),
            ci_pending: self.ci_pending.compile(),
            changes_requested: self.changes_requested.compile(),
            review_required: self.review_required.compile(),
            draft: self.draft.compile(),
            status_calculating: self.status_calculating.compile(),
            blocked_unknown: self.blocked_unknown.compile(),
            error: self.error.compile(),
        }
    }
}

impl TokenConfig {
    #[must_use]
    pub fn compile(&self) -> CompiledTokenConfig {
        CompiledTokenConfig {
            symbol: self.symbol.clone(),
            label: self.label.clone(),
            segments: compile_segments(&self.format),
        }
    }
}

impl ErrorConfig {
    #[must_use]
    pub fn compile(&self) -> CompiledErrorConfig {
        CompiledErrorConfig {
            symbol: self.symbol.clone(),
            segments: compile_segments(&self.format),
        }
    }
}
