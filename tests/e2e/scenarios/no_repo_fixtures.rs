use super::super::helpers::{TestEnv, setup_empty_dirs, write_executable};

/// git リポジトリ外シナリオ（`.git` のない空ディレクトリで実行）
pub fn without_git_remote() -> TestEnv {
    let (bin, home_tmp, repo) = setup_empty_dirs();
    let gh_script = "#!/bin/sh\necho 'gh should not be called' >&2\nexit 1\n";
    write_executable(bin.path().join("gh"), gh_script);
    TestEnv {
        bin,
        home_tmp,
        repo,
    }
}
