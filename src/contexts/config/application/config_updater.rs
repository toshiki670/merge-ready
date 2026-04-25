use super::super::domain::config::Config;

pub fn default_config() -> Config {
    let mut config = Config::default();
    config.fill_defaults();
    config
}
