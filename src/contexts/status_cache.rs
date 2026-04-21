//! Status cache context.
//!
//! This context manages the background daemon and cache
//! to keep `merge-ready prompt` responses low latency.
//!
//! ## Main responsibilities
//!
//! - Cache model and update logic (`domain::cache`, `application::cache`)
//! - Daemon state and lifecycle management (`domain::daemon`, `application::lifecycle`)
//! - Server/client IPC implementation (`infrastructure`)
//! - PID/socket/path management (`infrastructure::pid`, `infrastructure::paths`)
//! - CLI daemon command handling (`interface::cli::daemon`)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
