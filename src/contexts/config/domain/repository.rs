use super::config::Config;

pub trait ConfigRepository {
    fn load(&self) -> Config;

    /// # Errors
    /// Returns `io::Error` when the config path is unavailable or write fails.
    fn save(&self, config: &Config) -> Result<(), std::io::Error>;
}
