use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

pub fn is_git_repo(cwd: &Path) -> bool {
    let mut current = cwd;
    loop {
        if current.join(".git").exists() {
            return true;
        }
        match current.parent() {
            Some(p) => current = p,
            None => return false,
        }
    }
}

pub async fn current_branch(cwd: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn is_git_repo_returns_false_when_no_git_dir() {
        let dir = tempdir().unwrap();
        assert!(!is_git_repo(dir.path()));
    }
}
