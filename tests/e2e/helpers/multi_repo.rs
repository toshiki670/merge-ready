//! 複数リポジトリが同一 daemon を共有するシナリオ用の環境。

use std::fs;
use std::path::Path;
use tempfile::{TempDir, tempdir};

use super::{DaemonHandle, apply_coverage_env, run_prompt_with_timeout, write_executable};

pub struct MultiRepoEnv {
    pub bin: TempDir,
    pub home_tmp: TempDir,
    pub repo_a: TempDir,
    pub repo_b: TempDir,
}

impl MultiRepoEnv {
    /// `pr_view_a` / `pr_view_b` をそれぞれの `repo` に仕込み、
    /// `$PWD` ベースで応答する共有 `fake gh` をセットアップする。
    pub fn new(pr_view_a: &str, pr_view_b: &str) -> Self {
        let bin = tempdir().expect("bin");
        let home_tmp = tempdir().expect("home_tmp");
        let repo_a = tempdir().expect("repo_a");
        let repo_b = tempdir().expect("repo_b");

        let gh_script = "#!/bin/sh\n\
            case \"$*\" in\n\
              *'pr list'*)\n\
                cat \"$PWD/.gh_pr_list.json\"\n\
                ;;\n\
              *'pr checks'*)\n\
                printf '[{\"bucket\":\"pass\",\"state\":\"SUCCESS\"}]'\n\
                ;;\n\
              *'api'*'compare'*)\n\
                printf '{\"behind_by\":0}'\n\
                ;;\n\
              *)\n\
                printf 'unknown gh command: %s' \"$*\" >&2\n\
                exit 127\n\
                ;;\n\
            esac\n";
        write_executable(bin.path().join("gh"), gh_script);

        for (repo, json) in [(&repo_a, pr_view_a), (&repo_b, pr_view_b)] {
            let git_dir = repo.path().join(".git");
            fs::create_dir_all(&git_dir).expect("create .git");
            fs::write(git_dir.join("HEAD"), "ref: refs/heads/feat/my-feature\n")
                .expect("write HEAD");
            let inner = json.strip_prefix('{').unwrap_or(json);
            let list_json = format!(r#"[{{"number":1,{inner}]"#);
            fs::write(repo.path().join(".gh_pr_list.json"), &list_json)
                .expect("write response json");
        }

        Self {
            bin,
            home_tmp,
            repo_a,
            repo_b,
        }
    }

    pub fn path_env(&self) -> String {
        format!("{}:/bin:/usr/bin", self.bin.path().display())
    }

    pub fn home(&self) -> &Path {
        self.home_tmp.path()
    }

    /// daemon を `repo_a` の cwd で起動し、socket 出現まで最大 2000ms 待つ。
    pub fn start_daemon(&self) -> DaemonHandle {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
        let mut cmd = std::process::Command::new(&bin);
        cmd.args(["daemon", "start"])
            .env("PATH", self.path_env())
            .env("HOME", self.home())
            .env("TMPDIR", self.home())
            .env("MERGE_READY_BASE_DIR", self.home())
            .current_dir(self.repo_a.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        apply_coverage_env(&mut cmd);
        let mut child = cmd.spawn().expect("daemon spawn failed");

        let socket = self
            .home()
            .join(format!("daemon-{}.sock", env!("CARGO_PKG_VERSION")));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if socket.exists() {
                return DaemonHandle::new(child, self.home().to_path_buf());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = child.kill();
        let _ = child.wait();
        panic!("daemon did not start within 2000ms");
    }

    /// `repo` の prompt 出力が `"? loading"` でなくなるまで最大 `max_ms` ms 待つ。
    pub fn wait_for_cache_in(&self, repo: &TempDir, max_ms: u64) {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready-prompt");
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
        loop {
            let mut cmd = std::process::Command::new(&bin);
            cmd.env("PATH", self.path_env())
                .env("HOME", self.home())
                .env("TMPDIR", self.home())
                .env("MERGE_READY_BASE_DIR", self.home())
                .current_dir(repo.path());
            apply_coverage_env(&mut cmd);
            let out = run_prompt_with_timeout(&mut cmd);
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            if stdout != "? loading" {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "cache not populated within {max_ms}ms"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    /// `repo` から `merge-ready-prompt` を実行してその出力を返す。
    pub fn prompt_output(&self, repo: &TempDir) -> String {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready-prompt");
        let mut cmd = std::process::Command::new(&bin);
        cmd.env("PATH", self.path_env())
            .env("HOME", self.home())
            .env("TMPDIR", self.home())
            .env("MERGE_READY_BASE_DIR", self.home())
            .current_dir(repo.path());
        apply_coverage_env(&mut cmd);
        let out = run_prompt_with_timeout(&mut cmd);
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
}
