use super::super::helpers::{TestEnv, setup_git_dirs, write_executable};

/// compare API が `behind_by` を返すシナリオ用の `fake gh` を配置する。
pub fn with_behind_by(pr_view_json: &str, pr_checks_json: Option<&str>, behind_by: u64) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");

    let checks_block = match pr_checks_json {
        Some(j) => format!("printf '%s' '{j}'\n"),
        None => "printf 'unexpected pr checks call' >&2\nexit 1\n".to_string(),
    };

    let inner = pr_view_json.strip_prefix('{').unwrap_or(pr_view_json);
    let pr_list_json = format!(r#"[{{"number":1,{inner}]"#);

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *'pr list'*)\n\
             printf '%s' '{pr_list_json}'\n\
             ;;\n\
           *'pr checks'*)\n\
             {checks_block}\
             ;;\n\
           *'api'*'compare'*)\n\
             printf '{{\"behind_by\":{behind_by}}}'\n\
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

/// compare API がエラーを返すシナリオ用の `fake gh` を配置する。
pub fn with_compare_error(pr_view_json: &str, pr_checks_json: Option<&str>) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");

    let checks_block = match pr_checks_json {
        Some(j) => format!("printf '%s' '{j}'\n"),
        None => "printf 'unexpected pr checks call' >&2\nexit 1\n".to_string(),
    };

    let inner = pr_view_json.strip_prefix('{').unwrap_or(pr_view_json);
    let pr_list_json = format!(r#"[{{"number":1,{inner}]"#);

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *'pr list'*)\n\
             printf '%s' '{pr_list_json}'\n\
             ;;\n\
           *'pr checks'*)\n\
             {checks_block}\
             ;;\n\
           *'api'*'compare'*)\n\
             printf 'API error' >&2\n\
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

/// `gh pr list` / `gh pr checks` が成功するが compare API が
/// exit 0 で不正な JSON を返すシナリオ（`GhCompare` パース失敗で `None`）
pub fn with_invalid_compare_json(pr_view_json: &str, pr_checks_json: &str) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let inner = pr_view_json.strip_prefix('{').unwrap_or(pr_view_json);
    let pr_list_json = format!(r#"[{{"number":1,{inner}]"#);
    let checks_json = pr_checks_json.to_owned();
    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *'pr list'*)\n\
             printf '%s' '{pr_list_json}'\n\
             ;;\n\
           *'pr checks'*)\n\
             printf '%s' '{checks_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf 'not-valid-json'\n\
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
