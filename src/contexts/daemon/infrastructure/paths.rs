use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    base_dir().join("daemon.sock")
}

pub fn pid_path() -> PathBuf {
    base_dir().join("daemon.pid")
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
