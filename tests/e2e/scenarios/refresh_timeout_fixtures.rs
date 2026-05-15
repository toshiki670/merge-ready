use std::path::PathBuf;

use super::super::helpers::{TestEnv, setup_git_dirs, write_executable};

/// 初回 pr list は即時応答し、2 回目の pr list は 2 秒 sleep してから応答する fixture。
///
/// `clear_refresh_lock` のシナリオ用。
/// - 初回フェッチ (pr list #1) が素早く完了 → `CacheEntry` に output が入り `is_active()=true` になる。
/// - スケジューラが次のリフレッシュをスケジュール → pr list #2 がスロー (2s sleep)。
/// - `refresh_lock_timeout=1s` を超えると `clear_refresh_lock` が呼ばれ、スケジューラがリトライ。
/// - pr list #3 以降は即時応答 → リトライ成功。
///
/// カウンタファイルは `pr list` 呼び出し専用。`home_tmp` 配下に置く。
pub fn with_slow_second_pr_list() -> (TestEnv, PathBuf) {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let log_path = home_tmp.path().join("gh_calls.log");
    let log = log_path.display().to_string();
    let pr_list_counter_path = home_tmp.path().join("pr_list_counter");
    let pr_list_counter = pr_list_counter_path.display().to_string();

    let pr_list_json = r#"[{"number":1,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}]"#;
    let checks_json = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

    let script = format!(
        "#!/bin/sh\n\
         printf '1' >> \"{log}\"\n\
         case \"$*\" in\n\
           *'pr list'*)\n\
             pl=$(cat \"{pr_list_counter}\" 2>/dev/null || printf '0')\n\
             pl=$((pl + 1))\n\
             printf '%d' \"$pl\" > \"{pr_list_counter}\"\n\
             if [ \"$pl\" -eq 2 ]; then\n\
               sleep 2\n\
             fi\n\
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
