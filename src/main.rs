mod cached;
mod refresh;

use merge_readiness_application::prompt::{ExecutionMode, RepoIdPort};
use merge_readiness_infrastructure::{gh::GhClient, logger::Logger};
use merge_readiness_interface::cli;

struct InfraRepoIdPort;

impl RepoIdPort for InfraRepoIdPort {
    fn get(&self) -> Option<String> {
        merge_readiness_infrastructure::repo_id::get()
    }
}

fn main() {
    let Some(mode) = cli::run(&InfraRepoIdPort) else {
        return;
    };
    match mode {
        ExecutionMode::Direct => {
            cli::prompt::direct::run(&GhClient::new(), &Logger);
        }
        ExecutionMode::Cached => cached::run(),
        ExecutionMode::BackgroundRefresh { repo_id } => refresh::run(&repo_id),
    }
}
