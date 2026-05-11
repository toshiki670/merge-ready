use super::super::helpers::{TestEnv, setup_git_dirs, write_executable};

/// CI 未設定シナリオ: `gh pr checks` が `"no checks reported"` で `exit 1` を返す。
pub fn with_no_ci_checks(pr_view_json: &str) -> TestEnv {
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
             printf \"%s\" \"no checks reported on the 'test-branch' branch\" >&2\n\
             exit 1\n\
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
