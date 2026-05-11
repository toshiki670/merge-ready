use super::super::helpers::{TestEnv, setup_git_dirs, write_executable};

/// terminal PR シナリオ（呼び出しカウンタ付き）
///
/// `gh pr list` が closed / merged JSON を返す。カウンタログファイルのパスを返す。
pub fn with_terminal_pr_call_log(state: &str) -> (TestEnv, std::path::PathBuf) {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let log_path = home_tmp.path().join("gh_calls.log");
    let log = log_path.display().to_string();
    let pr_list_json = format!(
        r#"[{{"number":1,"state":"{state}","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}}]"#
    );
    let script = format!(
        "#!/bin/sh\n\
         printf '1' >> \"{log}\"\n\
         case \"$*\" in\n\
           *'pr list'*)\n\
             printf '%s' '{pr_list_json}'\n\
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
