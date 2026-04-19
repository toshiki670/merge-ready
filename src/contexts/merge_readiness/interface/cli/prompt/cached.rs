use crate::contexts::merge_readiness::application::prompt::RepoIdPort;

pub fn run(repo_id_port: &impl RepoIdPort, query: impl Fn(&str) -> Option<String>) {
    let Some(id) = repo_id_port.get() else { return };
    match query(&id) {
        Some(s) => print!("{s}"),
        None => print!("? loading"),
    }
}
