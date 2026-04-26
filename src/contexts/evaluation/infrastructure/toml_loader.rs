use std::path::PathBuf;

use crate::contexts::evaluation::domain::display_config::{DisplayConfig, DisplayConfigRepository};

pub struct TomlConfigRepository;

#[allow(clippy::unused_self)]
impl DisplayConfigRepository for TomlConfigRepository {
    fn load(&self) -> DisplayConfig {
        let Some(path) = config_path() else {
            return DisplayConfig::default();
        };
        let Ok(content) = std::fs::read_to_string(path) else {
            return DisplayConfig::default();
        };
        toml::from_str(&content).unwrap_or_default()
    }
}

// XDG_CONFIG_HOME が設定されていればそちらを優先し、なければ $HOME/.config にフォールバックする。
pub(crate) fn config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("merge-ready.toml"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config").join("merge-ready.toml"))
}
