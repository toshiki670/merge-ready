use super::config::Config;

pub trait ConfigRepository {
    fn load(&self) -> Config;
}
