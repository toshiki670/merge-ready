//! Git サブディレクトリ探索シナリオ
//!
//! `.git` の直親でなく、その内部サブディレクトリから実行した場合でも
//! `is_git_repo` が親ディレクトリを遡って `.git` を検出することを確認する。
//!
//! カバー:
//! - `evaluation::infrastructure::git::is_git_repo` の `Some(p) => current = p` 分岐

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::cargo::cargo_bin;

use super::super::helpers::{DaemonHandle, TestEnv, run_prompt_with_timeout};

const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// リポジトリのサブディレクトリから実行しても PR 出力が得られる
#[test]
fn test_prompt_from_git_subdirectory() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    // repo 内にサブディレクトリを作成
    let subdir = env.repo.path().join("src").join("nested");
    std::fs::create_dir_all(&subdir).expect("create subdir");

    let _daemon = DaemonHandle::start(&env);

    let bin = cargo_bin(PROMPT_BIN);

    // サブディレクトリから merge-ready-prompt を実行
    // is_git_repo が .git を祖先ディレクトリで検出するパスを通る
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(5000);
    loop {
        let out = run_prompt_with_timeout(
            std::process::Command::new(&bin)
                .env("PATH", env.path_env())
                .env("HOME", env.home())
                .env("TMPDIR", env.home())
                .current_dir(&subdir),
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        let s = stdout.trim();
        if s != "? loading" {
            assert!(
                out.status.success(),
                "merge-ready-prompt should succeed from subdir"
            );
            // サブディレクトリでも PR 出力が得られること
            assert!(!s.is_empty(), "subdir から実行しても PR 出力が得られるはず");
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "subdir からの PR 出力が 5000ms 以内に得られなかった"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
