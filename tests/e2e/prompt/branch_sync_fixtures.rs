use super::super::helpers::{TestEnv, graphql_single, setup_git_dirs, write_executable};

/// compare API が `behind_by` を返すシナリオ用の `fake gh` を配置する。
///
/// PR メタ + CI は単一 graphql 応答、branch sync の `behind_by` は REST compare 併用。
pub fn with_behind_by(pr_view_json: &str, pr_checks_json: Option<&str>, behind_by: u64) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");

    let graphql_json = graphql_single(pr_view_json, pr_checks_json);

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf '{{\"behind_by\":{behind_by}}}'\n\
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

/// compare API がエラーを返すシナリオ用の `fake gh` を配置する。
pub fn with_compare_error(pr_view_json: &str, pr_checks_json: Option<&str>) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");

    let graphql_json = graphql_single(pr_view_json, pr_checks_json);

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf 'API error' >&2\n\
             exit 1\n\
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

/// graphql は成功するが compare API が exit 0 で不正な JSON を返すシナリオ
/// （`GhCompare` パース失敗で `behind_by` は `None` → `SyncUnknown`）。
pub fn with_invalid_compare_json(pr_view_json: &str, pr_checks_json: &str) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");

    let graphql_json = graphql_single(pr_view_json, Some(pr_checks_json));

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf 'not-valid-json'\n\
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
