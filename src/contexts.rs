//! Bounded contexts for `merge-ready`.
//!
//! This module exposes three contexts, each separated by responsibility.
//!
//! - [`configuration`] - manages display and behavior settings
//! - [`merge_readiness`] - evaluates pull request merge readiness
//! - [`status_cache`] - provides low-latency responses via daemon/cache
//!
//! # Context relationship
//!
//! ```text
//! CLI (`merge-ready prompt`)
//!   -> status_cache (serve quickly from daemon/cache)
//!   -> merge_readiness (fetch + evaluate merge readiness)
//!   -> configuration (presentation and behavior settings)
//! ```

pub mod configuration;
pub mod merge_readiness;
pub mod status_cache;
