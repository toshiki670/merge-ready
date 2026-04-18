pub enum CacheState {
    Fresh(String),
    Stale(String),
    Miss,
}

pub trait CachePort {
    fn check(&self, repo_id: &str) -> CacheState;
}
