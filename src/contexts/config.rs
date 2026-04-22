//! Configuration context.
//!
//! This context handles reading and updating `merge-ready` configuration.
//! It provides settings related to token presentation and CLI behavior.
//!
//! ## Main responsibilities
//!
//! - Configuration retrieval service (`application::config_service`)
//! - Configuration update use case (`application::config_updater`)
//! - Configuration model and repository boundary (`domain`)
//! - TOML-based persistence (`infrastructure::toml_loader`)
//! - CLI entry points (`interface::cli`)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
