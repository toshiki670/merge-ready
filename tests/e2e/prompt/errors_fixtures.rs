use super::super::helpers::{TestEnv, graphql_response, pr_node, setup_git_dirs, write_executable};

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

/// `gh api graphql` が exit 0 で不正な JSON を返すシナリオ
pub fn with_invalid_graphql_json() -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let script = "#!/bin/sh\n\
                  case \"$*\" in\n\
                    *graphql*)\n\
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

/// graphql 応答の封筒形状は正しいが `statusCheckRollup` の型が壊れているシナリオ。
/// `GhPrNode` のデシリアライズが失敗し、`✗ unexpected error` になることを検証する。
pub fn with_invalid_rollup_json(pr_view_json: &str) -> TestEnv {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");

    // 妥当な PR ノードを作り、rollup をオブジェクト以外（文字列）に差し替えて壊す。
    let mut node = pr_node(1, pr_view_json, None);
    node["commits"]["nodes"][0]["commit"]["statusCheckRollup"] = serde_json::json!("not-an-object");
    let graphql_json = graphql_response(&[node], "main");

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *graphql*)\n\
             printf '%s' '{graphql_json}'\n\
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
