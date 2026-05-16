use std::path::PathBuf;

/// ソケット・PID ファイル名に含めるバージョン。
///
/// バージョンごとに別ファイルを使うことで、複数バージョンのデーモンが共存しても
/// 衝突せず、新デーモンが古いデーモンを安全に検知・停止できる。
const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

const SOCKET_PREFIX: &str = "daemon-";
const SOCKET_EXT: &str = ".sock";
const PID_EXT: &str = ".pid";

#[derive(Clone)]
pub struct Paths {
    base_dir: PathBuf,
}

impl Paths {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn socket_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{SOCKET_PREFIX}{DAEMON_VERSION}{SOCKET_EXT}"))
    }

    pub fn pid_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{SOCKET_PREFIX}{DAEMON_VERSION}{PID_EXT}"))
    }

    pub fn lock_path(&self) -> PathBuf {
        self.base_dir.join("daemon.lock")
    }

    /// 旧バージョンの PID ファイルを列挙する（現バージョンは除外）。
    ///
    /// 列挙対象は `daemon-{version}.pid` 形式のファイルのみ。新デーモン起動時に
    /// 旧バージョンの停止・stale ファイル削除のために使用する。
    pub fn old_daemon_pid_files(&self) -> Vec<PathBuf> {
        let current_pid_name = format!("{SOCKET_PREFIX}{DAEMON_VERSION}{PID_EXT}");
        std::fs::read_dir(&self.base_dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    n.starts_with(SOCKET_PREFIX) && n.ends_with(PID_EXT) && n != current_pid_name
                })
            })
            .collect()
    }
}

impl Default for Paths {
    fn default() -> Self {
        let base = std::env::var("MERGE_READY_BASE_DIR").map_or_else(|_| base_dir(), PathBuf::from);
        Self::new(base)
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
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(
            p.socket_path(),
            std::path::PathBuf::from(format!("/tmp/test-merge-ready/daemon-{version}.sock"))
        );
        assert_eq!(
            p.pid_path(),
            std::path::PathBuf::from(format!("/tmp/test-merge-ready/daemon-{version}.pid"))
        );
        assert_eq!(
            p.lock_path(),
            std::path::PathBuf::from("/tmp/test-merge-ready/daemon.lock")
        );
    }

    #[test]
    fn old_daemon_pid_files_excludes_current_version() {
        let dir = tempfile::tempdir().unwrap();
        let version = env!("CARGO_PKG_VERSION");

        // 現バージョン + 旧バージョン 2 つ + 無関係なファイル
        std::fs::write(dir.path().join(format!("daemon-{version}.pid")), "1").unwrap();
        std::fs::write(dir.path().join("daemon-0.0.0.pid"), "2").unwrap();
        std::fs::write(dir.path().join("daemon-0.1.0.pid"), "3").unwrap();
        std::fs::write(dir.path().join("daemon.lock"), "").unwrap();
        std::fs::write(dir.path().join("other.pid"), "").unwrap();

        let paths = Paths::new(dir.path().to_path_buf());
        let mut found: Vec<String> = paths
            .old_daemon_pid_files()
            .into_iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        found.sort();
        assert_eq!(found, vec!["daemon-0.0.0.pid", "daemon-0.1.0.pid"]);
    }
}
