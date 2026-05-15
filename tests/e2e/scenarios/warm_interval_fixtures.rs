use std::path::PathBuf;

use super::super::helpers::{TestEnv, setup_git_dirs, write_executable};

/// Warm (CLEAN) PR + gh 呼び出しカウントログ付き fixture。
///
/// Warm 間隔バリアントのシナリオで使用する。
pub fn with_warm_pr_call_log() -> (TestEnv, PathBuf) {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let log_path = home_tmp.path().join("gh_calls.log");
    let log = log_path.display().to_string();
    let pr_list_json = r#"[{"number":1,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}]"#;
    let checks_json = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;
    let script = format!(
        "#!/bin/sh\n\
         printf '1' >> \"{log}\"\n\
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
