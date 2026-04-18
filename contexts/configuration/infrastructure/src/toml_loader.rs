use configuration_domain::{config::Config, repository::ConfigRepository};

pub struct TomlConfigRepository;

impl ConfigRepository for TomlConfigRepository {
    fn load(&self) -> Config {
        let Some(home) = std::env::var_os("HOME") else {
            return Config::default();
        };
        let path = std::path::PathBuf::from(home)
            .join(".config")
            .join("merge-ready.toml");
        let Ok(content) = std::fs::read_to_string(path) else {
            return Config::default();
        };
        toml::from_str(&content).unwrap_or_default()
    }
}
