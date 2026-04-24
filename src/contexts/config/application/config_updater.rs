use super::super::domain::config::{CURRENT_VERSION, Config};
use super::port::UpdateConfigPort;

pub fn run(port: &impl UpdateConfigPort) -> Result<(), std::io::Error> {
    let mut config = port.load();
    if config.version == CURRENT_VERSION {
        return Ok(());
    }
    config.version = CURRENT_VERSION;
    config.fill_defaults();
    port.save(&config)
}

pub fn default_config() -> Config {
    let mut config = Config {
        version: CURRENT_VERSION,
        ..Config::default()
    };
    config.fill_defaults();
    config
}
