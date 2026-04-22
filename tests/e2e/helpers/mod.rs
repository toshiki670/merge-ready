mod daemon_handle;
mod env;
mod multi_repo;

pub use daemon_handle::{DaemonHandle, FakeDaemonHandle};
pub use env::TestEnv;
pub use multi_repo::MultiRepoEnv;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

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
