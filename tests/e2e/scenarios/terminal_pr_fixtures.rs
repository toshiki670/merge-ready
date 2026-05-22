use super::super::helpers::{
    FAKE_GH_RATE_LIMIT_OK_SNIPPET, TestEnv, graphql_single, setup_git_dirs, write_executable,
};

/// terminal PR シナリオ（呼び出しカウンタ付き）
///
/// graphql 応答が closed / merged PR を返す。カウンタログファイルのパスを返す。
pub fn with_terminal_pr_call_log(state: &str) -> (TestEnv, std::path::PathBuf) {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let log_path = home_tmp.path().join("gh_calls.log");
    let log = log_path.display().to_string();
    let fragment = format!(
        r#"{{"state":"{state}","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}}"#
    );
    // closed / merged は OPEN フィルタで除外され CI 評価されないため rollup は null。
    let graphql_json = graphql_single(&fragment, None);
    let script = format!(
        "#!/bin/sh\n\
         {FAKE_GH_RATE_LIMIT_OK_SNIPPET}\
         printf '1' >> \"{log}\"\n\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
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
