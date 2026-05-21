use super::super::helpers::{TestEnv, graphql_single, setup_git_dirs, write_executable};

/// CI 未設定シナリオ: `statusCheckRollup` が `null`（チェック未設定）の graphql 応答。
pub fn with_no_ci_checks(pr_view_json: &str) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");

    let graphql_json = graphql_single(pr_view_json, None);

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf '{{\"behind_by\":0}}'\n\
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
