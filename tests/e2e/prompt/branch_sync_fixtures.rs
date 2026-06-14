use std::path::PathBuf;

use super::super::helpers::{
    FAKE_GH_RATE_LIMIT_OK_SNIPPET, TestEnv, graphql_single, setup_git_dirs, write_executable,
};

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

/// compare API が呼ばれた回数を記録する `fake gh` を配置する（#410）。
///
/// graphql は常に成功し、`api ... compare ...` が呼ばれたときだけ
/// `compare_calls.log` に 1 文字追記する。Calculating / CONFLICTING のように
/// compare をスキップすべきケースで「ログ未生成（= 呼び出しゼロ）」を
/// 検証するために使う。`behind_by` は使われないはずだが、念のため `1` を返す。
pub fn with_compare_call_log(
    pr_view_json: &str,
    pr_checks_json: Option<&str>,
) -> (TestEnv, PathBuf) {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let log_path = home_tmp.path().join("compare_calls.log");
    let log = log_path.display().to_string();

    let graphql_json = graphql_single(pr_view_json, pr_checks_json);

    let script = format!(
        "#!/bin/sh\n\
         {FAKE_GH_RATE_LIMIT_OK_SNIPPET}\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf '1' >> \"{log}\"\n\
             printf '{{\"behind_by\":1}}'\n\
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
