mod args;
pub mod direct;

pub use args::{AFTER_HELP, PromptArgs};

use crate::contexts::merge_readiness::application::prompt::{ExecutionMode, RepoIdPort};

pub fn resolve_mode(args: &PromptArgs, repo_id_port: &impl RepoIdPort) -> Option<ExecutionMode> {
    if args.refresh {
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
        let mode = resolve_mode(&args(true, false, Some("abc")), &FixedRepoIdPort(None));
        assert!(
            matches!(mode, Some(ExecutionMode::BackgroundRefresh { repo_id }) if repo_id == "abc")
        );
    }

    #[test]
    fn refresh_falls_back_to_port_when_no_repo_id_arg() {
        let mode = resolve_mode(
            &args(true, false, None),
            &FixedRepoIdPort(Some("from-port")),
        );
        assert!(
            matches!(mode, Some(ExecutionMode::BackgroundRefresh { repo_id }) if repo_id == "from-port")
        );
    }

    #[test]
    fn refresh_returns_none_when_repo_id_unavailable() {
        let mode = resolve_mode(&args(true, false, None), &FixedRepoIdPort(None));
        assert!(mode.is_none());
    }

    #[test]
    fn no_cache_returns_direct() {
        let mode = resolve_mode(&args(false, true, None), &FixedRepoIdPort(None));
        assert!(matches!(mode, Some(ExecutionMode::Direct)));
    }

    #[test]
    fn default_returns_cached() {
        let mode = resolve_mode(&args(false, false, None), &FixedRepoIdPort(None));
        assert!(matches!(mode, Some(ExecutionMode::Cached)));
    }
}
