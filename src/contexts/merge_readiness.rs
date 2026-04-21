//! Merge readiness context.
//!
//! 現在ブランチに紐づく Pull Request のマージ可否を判定する中核コンテキストです。
//! GitHub から収集した状態をポリシーで評価し、表示トークンへ正規化します。
//!
//! ## Main responsibilities
//!
//! - PR ライフサイクル判定 (`domain::pr_state`)
//! - ブランチ同期状態判定 (`domain::branch_sync`)
//! - CI チェック集約 (`domain::ci_checks`)
//! - レビュー状態判定 (`domain::review`)
//! - 最終判定ポリシー (`domain::policy`, `domain::merge_ready`)
//! - 表示トークンへの変換 (`application`)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
