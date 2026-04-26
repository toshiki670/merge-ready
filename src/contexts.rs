//! Bounded contexts for `merge-ready`.
//!
//! This module exposes three contexts, each separated by responsibility.
//!
//! - [`config`] - manages display and behavior settings
//! - [`evaluation`] - evaluates pull request merge readiness
//! - [`daemon`] - provides low-latency responses via daemon/cache
//!
//! # Context relationship
//!
//! ```text
//! merge-ready-prompt (lightweight bin)
//!   -> daemon (serve quickly from daemon/cache)
//!   -> evaluation (fetch + evaluate merge readiness)
//!   -> config (presentation and behavior settings)
//! ```

pub mod daemon;
pub mod evaluation;
