use super::super::helpers::{TestEnv, setup_git_dirs, write_executable};

/// PR なしシナリオ（遅延付き）: stale refresh 中の挙動を再現するため、初回呼び出しは即座に返し、
/// 2回目以降（stale refresh）のみ `delay_ms` 遅延させる。
pub fn with_no_pr_stale_delay_ms(delay_ms: u64) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let secs = delay_ms / 1000;
    let millis = delay_ms % 1000;
    let sleep_arg = format!("{secs}.{millis:03}");
    let count_path = home_tmp.path().join(".gh_call_count").display().to_string();
    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *'pr list'*)\n\
             count=$(cat \"{count_path}\" 2>/dev/null || printf '0')\n\
             count=$((count + 1))\n\
             printf '%d' \"$count\" > \"{count_path}\"\n\
             if [ \"$count\" -gt 1 ]; then\n\
                 sleep {sleep_arg}\n\
             fi\n\
             printf '[]'\n\
             ;;\n\
           *'repo view'*'defaultBranchRef'*)\n\
             printf '{{\"defaultBranchRef\":{{\"name\":\"main\"}}}}'\n\
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
