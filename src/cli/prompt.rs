mod args;
mod cached;
mod direct;
mod refresh;

pub(super) use args::{AFTER_HELP, PromptArgs};

use crate::application::prompt::{ExecutionMode, RepoIdPort};

pub(crate) fn run(args: &PromptArgs) {
    let Some(mode) = execution_mode(args, &InfraRepoIdPort) else {
        return;
    };
    match mode {
        ExecutionMode::Direct => direct::run_direct(),
        ExecutionMode::Cached => cached::run_cached(),
        ExecutionMode::BackgroundRefresh { repo_id } => refresh::run_refresh(&repo_id),
    }
}

pub(super) struct InfraRepoIdPort;

impl RepoIdPort for InfraRepoIdPort {
    fn get(&self) -> Option<String> {
        crate::infra::repo_id::get()
    }
}

fn execution_mode(args: &PromptArgs, repo_id_port: &impl RepoIdPort) -> Option<ExecutionMode> {
    if args.refresh {
        // 親プロセスから渡された repo_id を優先し、なければ git から取得
        let repo_id = args.repo_id.clone().or_else(|| repo_id_port.get())?;
        return Some(ExecutionMode::BackgroundRefresh { repo_id });
    }
    if args.no_cache {
        return Some(ExecutionMode::Direct);
    }
    Some(ExecutionMode::Cached)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedRepoIdPort(Option<&'static str>);

    impl RepoIdPort for FixedRepoIdPort {
        fn get(&self) -> Option<String> {
            self.0.map(str::to_owned)
        }
    }

    fn args(refresh: bool, no_cache: bool, repo_id: Option<&'static str>) -> PromptArgs {
        PromptArgs {
            refresh,
            no_cache,
            repo_id: repo_id.map(str::to_owned),
        }
    }

    #[test]
    fn refresh_with_explicit_repo_id() {
        let mode = execution_mode(&args(true, false, Some("abc")), &FixedRepoIdPort(None));
        assert!(
            matches!(mode, Some(ExecutionMode::BackgroundRefresh { repo_id }) if repo_id == "abc")
        );
    }

    #[test]
    fn refresh_falls_back_to_port_when_no_repo_id_arg() {
        let mode = execution_mode(
            &args(true, false, None),
            &FixedRepoIdPort(Some("from-port")),
        );
        assert!(
            matches!(mode, Some(ExecutionMode::BackgroundRefresh { repo_id }) if repo_id == "from-port")
        );
    }

    #[test]
    fn refresh_returns_none_when_repo_id_unavailable() {
        let mode = execution_mode(&args(true, false, None), &FixedRepoIdPort(None));
        assert!(mode.is_none());
    }

    #[test]
    fn no_cache_returns_direct() {
        let mode = execution_mode(&args(false, true, None), &FixedRepoIdPort(None));
        assert!(matches!(mode, Some(ExecutionMode::Direct)));
    }

    #[test]
    fn default_returns_cached() {
        let mode = execution_mode(&args(false, false, None), &FixedRepoIdPort(None));
        assert!(matches!(mode, Some(ExecutionMode::Cached)));
    }
}
