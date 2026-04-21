//! Configuration context.
//!
//! `merge-ready` の設定値を取得・更新するコンテキストです。
//! トークン表示や CLI 挙動に関わる設定を提供します。
//!
//! ## Main responsibilities
//!
//! - 設定取得サービス (`application::config_service`)
//! - 設定更新ユースケース (`application::config_updater`)
//! - 設定モデルとリポジトリ境界 (`domain`)
//! - TOML ベース永続化 (`infrastructure::toml_loader`)
//! - CLI 入口 (`interface::cli`)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
