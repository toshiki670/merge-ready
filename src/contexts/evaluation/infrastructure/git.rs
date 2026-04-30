use std::path::Path;
use std::process::{Command, Stdio};

#[must_use]
pub fn is_git_repo(cwd: Option<&Path>) -> bool {
    let base = match cwd {
        Some(d) => d.to_path_buf(),
        None => match std::env::current_dir() {
            Ok(d) => d,
            Err(_) => return false,
        },
    };
    let mut current = base.as_path();
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

#[must_use]
pub fn current_branch(cwd: Option<&Path>) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(["branch", "--show-current"]);
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd.output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}
