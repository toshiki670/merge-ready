mod args;
pub mod cached;
pub mod direct;

pub use args::{AFTER_HELP, PromptArgs};

use crate::contexts::merge_readiness::application::prompt::ExecutionMode;

pub fn resolve_mode(args: &PromptArgs) -> ExecutionMode {
    if args.no_cache {
        return ExecutionMode::Direct;
    }
    ExecutionMode::Cached
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_cache_returns_direct() {
        let args = PromptArgs { no_cache: true };
        let mode = resolve_mode(&args);
        assert!(matches!(mode, ExecutionMode::Direct));
    }

    #[test]
    fn default_returns_cached() {
        let args = PromptArgs { no_cache: false };
        let mode = resolve_mode(&args);
        assert!(matches!(mode, ExecutionMode::Cached));
    }
}
