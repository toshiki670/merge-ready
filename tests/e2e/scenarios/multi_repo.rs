//! キャッシュライフサイクル シナリオ #6: 複数リポジトリの分離
//!
//! repo_a（Ready for merge）と repo_b（Resolve conflict）が同一 daemon を共有し、
//! キャッシュを汚染しない

use super::super::helpers::MultiRepoEnv;

/// #6: 同一 daemon が複数リポジトリのキャッシュを正しく分離すること
#[test]
fn test_daemon_multi_repo_isolation() {
    let env = MultiRepoEnv::new(
        // repo_a: merge-ready
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        // repo_b: conflict
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":null}"#,
    );
    let _daemon = env.start_daemon();

    env.wait_for_cache_in(&env.repo_a, 5000);
    env.wait_for_cache_in(&env.repo_b, 5000);

    let out_a = env.prompt_output(&env.repo_a);
    let out_b = env.prompt_output(&env.repo_b);

    assert_eq!(out_a, "✓ Ready for merge", "repo_a should be ready for merge");
    assert!(
        out_b.contains("Resolve conflict"),
        "repo_b should show Resolve conflict, got: {out_b}"
    );
    assert_ne!(out_a, out_b, "repos must not share cached output");
}
