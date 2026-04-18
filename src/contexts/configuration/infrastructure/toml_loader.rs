use std::path::PathBuf;

use crate::contexts::configuration::domain::{config::Config, repository::ConfigRepository};

pub struct TomlConfigRepository;

impl ConfigRepository for TomlConfigRepository {
    fn load(&self) -> Config {
        let Some(path) = config_path() else {
            return Config::default();
        };
        let Ok(content) = std::fs::read_to_string(path) else {
            return Config::default();
        };
        toml::from_str(&content).unwrap_or_default()
    }

    fn save(&self, config: &Config) -> Result<(), std::io::Error> {
        let path = config_path().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "config path not found")
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, content)
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
