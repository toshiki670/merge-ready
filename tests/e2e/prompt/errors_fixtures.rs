use super::super::helpers::{TestEnv, setup_git_dirs, write_executable};

/// `gh` バイナリが無期限にハングするシナリオ（タイムアウト検証用）
pub fn with_hanging_gh() -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    write_executable(bin.path().join("gh"), "#!/bin/sh\nsleep 9999\n");
    TestEnv {
        bin,
        home_tmp,
        repo,
    }
}

/// `gh pr list` が exit 0 で不正な JSON を返すシナリオ
pub fn with_invalid_pr_list_json() -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let script = "#!/bin/sh\n\
                  case \"$*\" in\n\
                    *'pr list'*)\n\
                      printf 'not-valid-json'\n\
                      ;;\n\
                    *)\n\
                      printf 'unknown gh command: %s' \"$*\" >&2\n\
                      exit 127\n\
                      ;;\n\
                  esac\n";
    write_executable(bin.path().join("gh"), script);
    TestEnv {
        bin,
        home_tmp,
        repo,
    }
}

/// `gh pr list` が成功するが `gh pr checks` が exit 0 で不正な JSON を返すシナリオ
pub fn with_invalid_pr_checks_json(pr_view_json: &str) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let inner = pr_view_json.strip_prefix('{').unwrap_or(pr_view_json);
    let pr_list_json = format!(r#"[{{"number":1,{inner}]"#);
    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *'pr list'*)\n\
             printf '%s' '{pr_list_json}'\n\
             ;;\n\
           *'pr checks'*)\n\
             printf 'not-valid-json'\n\
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
