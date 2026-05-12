use super::super::helpers::{TestEnv, setup_git_dirs, write_executable};

/// Terminal PR 再オープンシナリオ用 fixture。
///
/// 1 回目の `gh pr list` 呼び出しでは MERGED PR を返し、
/// 2 回目以降は OPEN + CLEAN + CI pass の PR を返す。
/// カウンタはホームディレクトリ内のファイルで管理する。
pub fn with_merged_then_open_pr() -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let home = home_tmp.path().display().to_string();

    let script = format!(
        "#!/bin/sh\n\
         COUNT_FILE=\"{home}/gh_call_count\"\n\
         count=$(cat \"$COUNT_FILE\" 2>/dev/null || printf '0')\n\
         count=$((count + 1))\n\
         printf '%s' \"$count\" > \"$COUNT_FILE\"\n\
         case \"$*\" in\n\
           *'pr list'*)\n\
             if [ \"$count\" -le 1 ]; then\n\
               printf '[{{\"number\":1,\"state\":\"MERGED\",\"isDraft\":false,\"mergeable\":\"UNKNOWN\",\"mergeStateStatus\":\"UNKNOWN\",\"reviewDecision\":null}}]'\n\
             else\n\
               printf '[{{\"number\":1,\"state\":\"OPEN\",\"isDraft\":false,\"mergeable\":\"MERGEABLE\",\"mergeStateStatus\":\"CLEAN\",\"reviewDecision\":null}}]'\n\
             fi\n\
             ;;\n\
           *'pr checks'*)\n\
             printf '[{{\"bucket\":\"pass\",\"state\":\"SUCCESS\",\"name\":\"ci\",\"link\":\"\"}}]'\n\
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
