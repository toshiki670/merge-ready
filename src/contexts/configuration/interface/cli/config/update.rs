use crate::contexts::configuration::application::{self, ConfigRepository};

pub fn run(repo: &impl ConfigRepository) {
    application::config_updater::run(repo);
}
