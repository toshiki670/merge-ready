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
}

// XDG_CONFIG_HOME が設定されていればそちらを優先し、なければ $HOME/.config にフォールバックする。
pub(crate) fn config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("merge-ready.toml"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config").join("merge-ready.toml"))
}

// バージョンが古い場合に設定ファイルをマイグレーションする。最新バージョンならファイルを変更しない。
pub(crate) fn migrate_config_if_needed(path: &std::path::Path) {
    use crate::contexts::configuration::domain::config::{CURRENT_VERSION, Config};

    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut config: Config = toml::from_str(&content).unwrap_or_default();

    if config.version == CURRENT_VERSION {
        return;
    }

    config.version = CURRENT_VERSION;
    config.fill_defaults();
    if let Ok(new_content) = toml::to_string_pretty(&config) {
        let _ = std::fs::write(path, new_content);
    }
}
