use super::super::helpers::{
    FAKE_GH_RATE_LIMIT_OK_SNIPPET, TestEnv, graphql_response, setup_git_dirs, write_executable,
};

/// デフォルトブランチ上シナリオ: 現在ブランチ == デフォルトブランチ、PR なし。
///
/// `branch` に "main" または "master" などを指定する。
/// graphql 応答は空 `nodes` + `defaultBranchRef.name = branch` を返す。
pub fn with_default_branch_no_pr(branch: &str) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs(branch);
    let graphql_json = graphql_response(&[], branch);
    let script = format!(
        "#!/bin/sh\n\
         {FAKE_GH_RATE_LIMIT_OK_SNIPPET}\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *)\n\
             printf 'unknown gh command: %s' \"$*\" >&2\n\
             exit 127\n\
             ;;\n\
         esac\n"
    );
    write_executable(bin.path().join("gh"), &script);
    TestEnv {
        bin,
        home_tmp,
        repo,
    }
}
