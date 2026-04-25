//! ホットパスのベンチマーク
//!
//! daemon + キャッシュウォーム状態での `merge-ready-prompt` 起動時間を計測する。
//! 目標: warm 起動 10ms 未満
//!
//! 使用方法:
//! ```
//! cargo bench --bench hot_path
//! ```

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;

struct BenchEnv {
    bin_dir: TempDir,
    tmp_dir: TempDir,
    repo_dir: TempDir,
    daemon_process: Option<std::process::Child>,
}

impl Drop for BenchEnv {
    fn drop(&mut self) {
        // daemon を停止する
        let bin = binary_path("merge-ready");
        let _ = Command::new(&bin)
            .args(["daemon", "stop"])
            .env("TMPDIR", self.tmp_dir.path())
            .output();
        if let Some(mut child) = self.daemon_process.take() {
            let _ = child.kill();
        }
    }
}

const FAKE_BRANCH: &str = "main";
const OPEN_MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

fn write_executable(path: &PathBuf, content: &str) {
    fs::write(path, content).expect("write");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn path_env(env: &BenchEnv) -> String {
    format!("{}:/bin:/usr/bin", env.bin_dir.path().display())
}

fn binary_path(name: &str) -> PathBuf {
    let mut path = std::env::current_exe().expect("current_exe");
    path.pop(); // hot_path-xxx
    path.pop(); // deps
    path.push(name);
    if !path.exists() {
        path.pop();
        path.pop();
        path.push("release");
        path.push(name);
    }
    path
}

fn dir_name() -> String {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata("/proc/self") {
            return format!("merge-ready-{}", meta.uid());
        }
    }
    "merge-ready".to_owned()
}

fn setup_bench_env() -> BenchEnv {
    let bin_dir = TempDir::new().expect("bin_dir");
    let tmp_dir = TempDir::new().expect("tmp_dir");
    let repo_dir = TempDir::new().expect("repo_dir");

    // fake gh（即時応答）
    let gh_script = "#!/bin/sh\ncase \"$*\" in\n  *'pr view'*) printf '%s' '".to_owned()
        + OPEN_MERGE_READY_JSON
        + "' ;;\n  *'pr checks'*) printf '%s' '"
        + CI_PASS_JSON
        + "' ;;\n  *'api'*'compare'*) printf '{\"behind_by\":0}' ;;\n  *) exit 0 ;;\nesac\n";
    write_executable(&bin_dir.path().join("gh"), &gh_script);

    // 最小限の git リポジトリ構造
    let git_dir = repo_dir.path().join(".git");
    fs::create_dir_all(&git_dir).expect("create .git");
    fs::write(
        git_dir.join("HEAD"),
        format!("ref: refs/heads/{FAKE_BRANCH}\n"),
    )
    .expect("write HEAD");

    let mut env = BenchEnv {
        bin_dir,
        tmp_dir,
        repo_dir,
        daemon_process: None,
    };

    // daemon を起動する
    let bin = binary_path("merge-ready");
    let child = Command::new(&bin)
        .args(["daemon", "start"])
        .env("PATH", path_env(&env))
        .env("TMPDIR", env.tmp_dir.path())
        .env("HOME", env.tmp_dir.path())
        .current_dir(env.repo_dir.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("daemon spawn");
    env.daemon_process = Some(child);

    // socket 出現を待つ
    let socket = env.tmp_dir.path().join(dir_name()).join("daemon.sock");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(3000);
    while std::time::Instant::now() < deadline {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(socket.exists(), "daemon did not start within 3s");

    // キャッシュが温まるまで待つ
    let prompt_bin = binary_path("merge-ready-prompt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(5000);
    loop {
        let out = Command::new(&prompt_bin)
            .env("PATH", path_env(&env))
            .env("TMPDIR", env.tmp_dir.path())
            .current_dir(env.repo_dir.path())
            .output()
            .expect("merge-ready-prompt failed");
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout != "? loading" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "cache did not warm within 5s"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    env
}

/// daemon + キャッシュウォーム状態での `merge-ready-prompt` 起動時間（目標: <10ms）
fn bench_cache_hit(c: &mut Criterion) {
    let env = setup_bench_env();
    let prompt_bin = binary_path("merge-ready-prompt");
    let path = path_env(&env);
    let tmpdir = env.tmp_dir.path().to_owned();
    let repo_dir = env.repo_dir.path().to_owned();

    c.bench_function("merge_ready_prompt_warm", |b| {
        b.iter(|| {
            Command::new(&prompt_bin)
                .env("PATH", &path)
                .env("TMPDIR", &tmpdir)
                .current_dir(&repo_dir)
                .output()
                .expect("merge-ready-prompt failed")
        });
    });
}

criterion_group!(benches, bench_cache_hit);
criterion_main!(benches);
