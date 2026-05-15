use std::path::PathBuf;

use super::super::helpers::{TestEnv, setup_git_dirs, write_executable};

/// 1 回目の gh 呼び出しは 5 秒 sleep してから応答し、2 回目以降は即時応答する fixture。
///
/// `clear_refresh_lock` のシナリオ用。カウンタファイルを `home_tmp` 配下に置く。
pub fn with_slow_first_call() -> (TestEnv, PathBuf) {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let log_path = home_tmp.path().join("gh_calls.log");
    let log = log_path.display().to_string();
    let counter_path = home_tmp.path().join("gh_counter");
    let counter = counter_path.display().to_string();

    let pr_list_json = r#"[{"number":1,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}]"#;
    let checks_json = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

    let script = format!(
        "#!/bin/sh\n\
         printf '1' >> \"{log}\"\n\
         count=$(cat \"{counter}\" 2>/dev/null || printf '0')\n\
         count=$((count + 1))\n\
         printf '%d' \"$count\" > \"{counter}\"\n\
         if [ \"$count\" -le 1 ]; then\n\
           sleep 5\n\
         fi\n\
         case \"$*\" in\n\
           *'pr list'*)\n\
             printf '%s' '{pr_list_json}'\n\
             ;;\n\
           *'pr checks'*)\n\
             printf '%s' '{checks_json}'\n\
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
