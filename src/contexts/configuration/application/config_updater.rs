use super::super::domain::config::CURRENT_VERSION;
use super::super::domain::repository::ConfigRepository;

pub fn run(repo: &impl ConfigRepository) {
    let mut config = repo.load();
    if config.version == CURRENT_VERSION {
        return;
    }
    config.version = CURRENT_VERSION;
    config.fill_defaults();
    repo.save(&config);
}
