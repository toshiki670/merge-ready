//! `MERGE_READY_LOG_LEVEL` シナリオ用 `fixture`（#413）。
//!
//! fake `gh` は `graphql`（refresh 本体）/ `api compare` を正常応答するが、
//! `api rate_limit` だけは exit 1 で失敗させる。これにより daemon の
//! `rate_limit` fetcher が `log::warn!("rate_limit fetch failed: ...")` を出す。
//! `rate_limit_log` で `api rate_limit` の呼び出し回数を記録し、
//! 「warn を出す経路を実際に通った」ことをテスト側から確認できるようにする。

use std::path::PathBuf;

use super::super::helpers::{
    ROLLUP_PASS, TestEnv, graphql_single, setup_git_dirs, write_executable,
};

pub struct LogLevelFixture {
    pub env: TestEnv,
    pub rate_limit_log: PathBuf,
}

/// `gh api rate_limit` が必ず失敗する fake `gh` を構築する。
pub fn with_failing_rate_limit() -> LogLevelFixture {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let rate_limit_log = home_tmp.path().join("rate_limit_calls.log");
    let rate_limit_log_s = rate_limit_log.display().to_string();

    let graphql_json = graphql_single(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(ROLLUP_PASS),
    );

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *'api rate_limit'*)\n\
             printf '1' >> \"{rate_limit_log_s}\"\n\
             printf 'rate_limit boom' >&2\n\
             exit 1\n\
             ;;\n\
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

    LogLevelFixture {
        env: TestEnv {
            bin,
            home_tmp,
            repo,
        },
        rate_limit_log,
    }
}
