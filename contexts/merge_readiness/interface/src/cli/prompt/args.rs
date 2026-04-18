use clap::Args;

pub const AFTER_HELP: &str = "Output tokens:
  ✓ merge-ready    Ready to merge
  ⚠ review         Review requested
  ⚠ ci-action      CI checks in progress
  ✗ ci-fail        CI checks failed
  ✗ conflict       Branch has merge conflicts
  ✗ update-branch  Branch is behind base branch
  ? sync-unknown   Branch sync status unknown";

#[derive(Args)]
pub struct PromptArgs {
    /// Bypass cache and fetch fresh data directly
    #[arg(long)]
    pub no_cache: bool,
    /// Fetch fresh data and update cache without displaying output
    #[arg(long, hide = true, conflicts_with = "no_cache")]
    pub refresh: bool,
    /// Repository ID for lock release (passed by parent process via --refresh)
    #[arg(long, hide = true, requires = "refresh")]
    pub repo_id: Option<String>,
}
