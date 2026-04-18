use crate::config::Config;

pub trait ConfigRepository {
    fn load(&self) -> Config;
}
