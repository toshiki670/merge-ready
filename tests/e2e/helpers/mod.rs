mod daemon_handle;
mod env;
mod multi_repo;

pub use daemon_handle::{DaemonHandle, FakeDaemonHandle};
pub use env::TestEnv;
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

/// macOS: `"merge-ready"`、Linux: `"merge-ready-{uid}"`
pub(crate) fn daemon_dir_name() -> String {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata("/proc/self") {
            return format!("merge-ready-{}", meta.uid());
        }
    }
    "merge-ready".to_owned()
}

pub(crate) fn write_executable(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::write(path, content).expect("failed to write script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("failed to chmod script");
}
