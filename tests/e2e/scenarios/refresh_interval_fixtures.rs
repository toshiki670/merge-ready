use std::path::PathBuf;

use super::super::helpers::{
    FAKE_GH_RATE_LIMIT_OK_SNIPPET, ROLLUP_PASS, TestEnv, graphql_single, setup_git_dirs,
    write_executable,
};

/// `mergeStateStatus="UNKNOWN"` → `State::Calculating` → `CacheHint::Hot` の PR。
/// gh 呼び出し回数をログファイルに記録する。
pub fn with_calculating_pr_call_log() -> (TestEnv, PathBuf) {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let log_path = home_tmp.path().join("gh_calls.log");
    let log = log_path.display().to_string();
    let graphql_json = graphql_single(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}"#,
        None,
    );
    let script = format!(
        "#!/bin/sh\n\
         {FAKE_GH_RATE_LIMIT_OK_SNIPPET}\
         printf '1' >> \"{log}\"\n\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf '{{\"behind_by\":0}}'\n\
             ;;\n\
           *)\n\
             printf 'unexpected gh call: %s' \"$*\" >&2\n\
             exit 127\n\
             ;;\n\
         esac\n"
    );
    write_executable(bin.path().join("gh"), &script);
    (
        TestEnv {
            bin,
            home_tmp,
            repo,
        },
        log_path,
    )
}

/// passing CI PR。gh 呼び出し回数をログファイルに記録する。
/// `mergeStateStatus="CLEAN"` + pass checks → `CacheHint::Warm`。
pub fn with_warm_pr_call_log() -> (TestEnv, PathBuf) {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let log_path = home_tmp.path().join("gh_calls.log");
    let log = log_path.display().to_string();
    let graphql_json = graphql_single(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(ROLLUP_PASS),
    );
    let script = format!(
        "#!/bin/sh\n\
         {FAKE_GH_RATE_LIMIT_OK_SNIPPET}\
         printf '1' >> \"{log}\"\n\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf '{{\"behind_by\":0}}'\n\
             ;;\n\
           *)\n\
             printf 'unexpected gh call: %s' \"$*\" >&2\n\
             exit 127\n\
             ;;\n\
         esac\n"
    );
    write_executable(bin.path().join("gh"), &script);
    (
        TestEnv {
            bin,
            home_tmp,
            repo,
        },
        log_path,
    )
}
