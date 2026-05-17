//! 複数 context / bin から参照される共有モジュール。
//!
//! DDD 4 層（domain/application/infrastructure/interface）からは独立しており、
//! どの層からも `use crate::shared::*` で参照してよい。

pub mod protocol;
pub mod refresh_mode;
