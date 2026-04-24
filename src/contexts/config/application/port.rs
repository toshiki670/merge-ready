use super::super::domain::config::Config;

pub trait LoadConfigPort {
    fn load(&self) -> Config;
}

pub trait UpdateConfigPort {
    fn load(&self) -> Config;
    fn save(&self, config: &Config) -> Result<(), std::io::Error>;
}
