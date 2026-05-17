use super::super::helpers::{
    FAKE_GH_RATE_LIMIT_OK_SNIPPET, TestEnv, setup_git_dirs, write_executable,
};

/// 1 回目の `pr list` 呼び出しは MERGED、2 回目以降は OPEN+CLEAN を返す stateful fixture。
///
/// カウンタファイルを `home_tmp` 配下に置き並列実行に対応する。
/// `api rate_limit` 呼び出しはカウンタを進めず静的に応答する（daemon の定期取得が
/// pr list カウンタを汚染しないようにするため）。
pub fn with_reopened_pr() -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let counter_path = home_tmp.path().join("pr_list_counter");
    let counter = counter_path.display().to_string();

    let merged_json = r#"[{"number":1,"state":"MERGED","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}]"#;
    let open_json = r#"[{"number":1,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}]"#;
    let checks_json = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

    let script = format!(
        "#!/bin/sh\n\
         {FAKE_GH_RATE_LIMIT_OK_SNIPPET}\
         case \"$*\" in\n\
           *'pr list'*)\n\
             count=$(cat \"{counter}\" 2>/dev/null || printf '0')\n\
             count=$((count + 1))\n\
             printf '%d' \"$count\" > \"{counter}\"\n\
             if [ \"$count\" -le 1 ]; then\n\
               printf '%s' '{merged_json}'\n\
             else\n\
               printf '%s' '{open_json}'\n\
             fi\n\
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
    TestEnv {
        bin,
        home_tmp,
        repo,
    }
}
