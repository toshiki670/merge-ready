use crate::contexts::config::application::{self, ConfigRepository};

pub fn run(repo: &impl ConfigRepository) -> Result<(), std::io::Error> {
    application::config_updater::run(repo)
}
