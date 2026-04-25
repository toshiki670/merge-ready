use super::super::domain::config::{CURRENT_VERSION, Config};
use super::super::domain::repository::ConfigRepository;

pub fn run(repo: &impl ConfigRepository) -> Result<(), std::io::Error> {
    let mut config = repo.load();
    if config.version == CURRENT_VERSION {
        return Ok(());
    }
    config.version = CURRENT_VERSION;
    config.fill_defaults();
    repo.save(&config)
}

pub fn default_config() -> Config {
    let mut config = Config {
        version: CURRENT_VERSION,
        ..Config::default()
    };
    config.fill_defaults();
    config
}
