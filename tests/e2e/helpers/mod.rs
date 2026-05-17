mod coverage;
mod daemon_handle;
mod env;
mod multi_repo;

pub(crate) use coverage::{apply_coverage_env, apply_coverage_env_assert};
pub use daemon_handle::DaemonHandle;
pub use env::TestEnv;
pub(crate) use env::{setup_empty_dirs, setup_git_dirs};
pub use multi_repo::MultiRepoEnv;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub(super) const PROMPT_TIMEOUT_MS: u64 = 5000;

/// `merge-ready-prompt` をタイムアウト付きで実行する。
///
/// `cmd` には `stdin`/`stdout`/`stderr` を設定せずに渡すこと（本関数内で設定する）。
/// `PROMPT_TIMEOUT_MS` 以内に完了しない場合はプロセスを kill してパニックする。
pub(super) fn run_prompt_with_timeout(cmd: &mut std::process::Command) -> std::process::Output {
    let mut child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn merge-ready-prompt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(PROMPT_TIMEOUT_MS);
    loop {
        if child.try_wait().is_ok_and(|s| s.is_some()) {
            return child.wait_with_output().expect("collect prompt output");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("merge-ready-prompt did not finish within {PROMPT_TIMEOUT_MS}ms");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

pub(crate) fn write_executable(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::write(path, content).expect("failed to write script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("failed to chmod script");
}

/// fake `gh` バイナリ用に `api rate_limit` を「クォータ十分」の静的 JSON で返す
/// シェルスクリプト断片。各 fixture の `case "$*"` よりも前に挿入することで、
/// daemon の `rate_limit` fetcher による予期しないコール記録を防ぐ。
///
/// reset を遠未来（2286 年）に設定しているため、スナップショットは常に「枯渇しない／
/// 残量比率ほぼ 1.0」と扱われ、既存テストの間隔判定に影響しない。
pub(crate) const FAKE_GH_RATE_LIMIT_OK_SNIPPET: &str = r#"case "$*" in
  *'api rate_limit'*)
    printf '%s' '{"resources":{"core":{"limit":5000,"remaining":4999,"reset":9999999999},"graphql":{"limit":5000,"remaining":4999,"reset":9999999999}}}'
    exit 0
    ;;
esac
"#;
