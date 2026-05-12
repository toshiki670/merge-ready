use std::path::PathBuf;

/// 内側デーモンプロセスを識別する環境変数名。
/// `interface::cli::daemon` の outer/inner 分岐および
/// `restart::spawn_self_as_daemon()` で参照する。
pub const DAEMON_INNER_ENV: &str = "MERGE_READY_DAEMON_INNER";

#[derive(Clone)]
pub struct Paths {
    base_dir: PathBuf,
}

impl Paths {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn socket_path(&self) -> PathBuf {
        self.base_dir.join("daemon.sock")
    }

    pub fn pid_path(&self) -> PathBuf {
        self.base_dir.join("daemon.pid")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.base_dir.join("daemon.lock")
    }
}

impl Default for Paths {
    fn default() -> Self {
        Self::new(base_dir())
    }
}

pub fn base_dir() -> PathBuf {
    std::env::temp_dir().join(dir_name())
}

fn dir_name() -> String {
    std::cfg_select! {
        target_os = "linux" => {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = std::fs::metadata("/proc/self") {
                format!("merge-ready-{}", meta.uid())
            } else {
                "merge-ready".to_owned()
            }
        },
        _ => "merge-ready".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::Paths;

    #[test]
    fn paths_from_custom_dir() {
        let p = Paths::new("/tmp/test-merge-ready".into());
        assert_eq!(
            p.socket_path(),
            std::path::PathBuf::from("/tmp/test-merge-ready/daemon.sock")
        );
        assert_eq!(
            p.pid_path(),
            std::path::PathBuf::from("/tmp/test-merge-ready/daemon.pid")
        );
        assert_eq!(
            p.lock_path(),
            std::path::PathBuf::from("/tmp/test-merge-ready/daemon.lock")
        );
    }
}
