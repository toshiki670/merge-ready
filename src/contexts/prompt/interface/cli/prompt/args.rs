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
pub struct PromptArgs {}
