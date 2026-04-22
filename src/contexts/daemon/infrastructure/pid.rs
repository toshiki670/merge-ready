use std::fs;

use super::paths;

pub fn write(pid: u32) {
    let path = paths::pid_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, pid.to_string());
}

#[must_use]
pub fn read() -> Option<u32> {
    fs::read_to_string(paths::pid_path())
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn remove() {
    let _ = fs::remove_file(paths::pid_path());
}

#[must_use]
pub fn is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
