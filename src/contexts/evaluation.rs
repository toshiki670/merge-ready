//! Merge readiness context.
//!
//! This is the core context that determines whether the pull request
//! for the current branch is ready to merge.
//! It evaluates GitHub-derived state through policies and normalizes it
//! into presentation tokens.
//!
//! ## Main responsibilities
//!
//! - PR lifecycle state evaluation (`domain::pr_state`)
//! - Branch sync state evaluation (`domain::branch_sync`)
//! - CI check aggregation (`domain::ci_checks`)
//! - Review status evaluation (`domain::review`)
//! - Final readiness policy evaluation (`domain::policy`, `domain::merge_ready`)
//! - Conversion to output tokens (`application`)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
