use super::super::domain::repository::ConfigRepository;

/// 設定更新ユースケース向けポート（interface 層が domain 型を直接参照しないための facade）。
pub trait UpdateConfigPort: ConfigRepository {}
impl<T: ConfigRepository> UpdateConfigPort for T {}
