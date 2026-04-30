use super::super::domain::display_config::{DisplayConfig, DisplayConfigRepository};

pub fn load(repo: &impl DisplayConfigRepository) -> DisplayConfig {
    repo.load()
}

#[must_use]
pub fn default_display_config() -> DisplayConfig {
    DisplayConfig::default()
}
