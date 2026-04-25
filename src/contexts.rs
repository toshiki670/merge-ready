//! Bounded contexts for `merge-ready`.
//!
//! This module exposes three contexts, each separated by responsibility.
//!
//! - [`config`] - manages display and behavior settings
//! - [`prompt`] - evaluates pull request merge readiness
//! - [`daemon`] - provides low-latency responses via daemon/cache
//!
//! # Context relationship
//!
//! ```text
//! merge-ready-prompt (lightweight bin)
//!   -> daemon (serve quickly from daemon/cache)
//!   -> prompt (fetch + evaluate merge readiness)
//!   -> config (presentation and behavior settings)
//! ```

pub mod config;
pub mod daemon;
pub mod prompt;
