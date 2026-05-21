use std::path::PathBuf;

use super::super::helpers::{
    FAKE_GH_RATE_LIMIT_OK_SNIPPET, ROLLUP_PASS, TestEnv, graphql_single, setup_git_dirs,
    write_executable,
};

/// 初回 graphql は即時応答し、2 回目の graphql は 2 秒 sleep してから応答する fixture。
///
/// `clear_refresh_lock` のシナリオ用。
/// - 初回フェッチ (graphql #1) が素早く完了 → `CacheEntry` に output が入り `is_active()=true` になる。
/// - スケジューラが次のリフレッシュをスケジュール → graphql #2 がスロー (2s sleep)。
/// - `refresh_lock_timeout=1s` を超えると `clear_refresh_lock` が呼ばれ、スケジューラがリトライ。
/// - graphql #3 以降は即時応答 → リトライ成功。
///
/// カウンタファイルは graphql 呼び出し専用。`home_tmp` 配下に置く。
pub fn with_slow_second_graphql() -> (TestEnv, PathBuf) {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let log_path = home_tmp.path().join("gh_calls.log");
    let log = log_path.display().to_string();
    let graphql_counter_path = home_tmp.path().join("graphql_counter");
    let graphql_counter = graphql_counter_path.display().to_string();

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
             gq=$(cat \"{graphql_counter}\" 2>/dev/null || printf '0')\n\
             gq=$((gq + 1))\n\
             printf '%d' \"$gq\" > \"{graphql_counter}\"\n\
             if [ \"$gq\" -eq 2 ]; then\n\
               sleep 2\n\
             fi\n\
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
