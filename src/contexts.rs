//! Bounded contexts for `merge-ready`.
//!
//! このモジュールは、アプリケーションを責務ごとに分割した
//! 3 つのコンテキストを公開します。
//!
//! - [`configuration`] - 表示・挙動の設定管理
//! - [`merge_readiness`] - PR のマージ可否判定
//! - [`status_cache`] - デーモンとキャッシュによる低遅延応答
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
