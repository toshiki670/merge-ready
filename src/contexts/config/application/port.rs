use super::super::domain::config::Config;

pub trait LoadConfigPort {
    fn load(&self) -> Config;
}

pub trait UpdateConfigPort: LoadConfigPort {
    /// # Errors
    /// Returns `io::Error` when persisting the configuration fails.
    fn save(&self, config: &Config) -> Result<(), std::io::Error>;
}
