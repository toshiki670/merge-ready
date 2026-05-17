use super::super::helpers::{
    FAKE_GH_RATE_LIMIT_OK_SNIPPET, TestEnv, setup_git_dirs, write_executable,
};

/// デフォルトブランチ上シナリオ: 現在ブランチ == デフォルトブランチ、PR なし。
///
/// `branch` に "main" または "master" などを指定する。
/// `gh pr list` → 空配列 `[]` を返す
/// `gh repo view --json defaultBranchRef` → `branch` をデフォルトブランチとして返す
pub fn with_default_branch_no_pr(branch: &str) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs(branch);
    let script = format!(
        "#!/bin/sh\n\
         {FAKE_GH_RATE_LIMIT_OK_SNIPPET}\
         case \"$*\" in\n\
           *'pr list'*)\n\
             printf '[]'\n\
             ;;\n\
           *'repo view'*'defaultBranchRef'*)\n\
             printf '{{\"defaultBranchRef\":{{\"name\":\"{branch}\"}}}}'\n\
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
