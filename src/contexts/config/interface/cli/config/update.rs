use crate::contexts::config::application::{config_updater, port::UpdateConfigPort};

pub fn run(port: &impl UpdateConfigPort) -> Result<(), std::io::Error> {
    config_updater::run(port)
}
