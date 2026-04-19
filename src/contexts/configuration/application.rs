pub mod config_service;
pub mod config_updater;

pub trait ConfigRepository: super::domain::repository::ConfigRepository {}

impl<T> ConfigRepository for T where T: super::domain::repository::ConfigRepository {}
