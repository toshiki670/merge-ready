//! Status cache context.
//!
//! `merge-ready prompt` の低遅延応答のために、
//! バックグラウンドデーモンとキャッシュを管理するコンテキストです。
//!
//! ## Main responsibilities
//!
//! - キャッシュモデルと更新ロジック (`domain::cache`, `application::cache`)
//! - デーモン状態・ライフサイクル管理 (`domain::daemon`, `application::lifecycle`)
//! - サーバー/クライアント IPC 実装 (`infrastructure`)
//! - PID/ソケット/パス管理 (`infrastructure::pid`, `infrastructure::paths`)
//! - CLI の daemon コマンド (`interface::cli::daemon`)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
