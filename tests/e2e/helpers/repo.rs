use std::fs;
use tempfile::{TempDir, tempdir};

pub(crate) fn setup_with_git(branch: &str) -> (TempDir, TempDir, TempDir) {
    let bin_dir = tempdir().expect("failed to create bin_dir");
    let home_dir = tempdir().expect("failed to create home_dir");
    let repo_dir = tempdir().expect("failed to create repo_dir");

    let git_dir = repo_dir.path().join(".git");
    fs::create_dir_all(git_dir.join("objects")).expect("create .git/objects");
    fs::create_dir_all(git_dir.join("refs")).expect("create .git/refs");
    fs::write(git_dir.join("HEAD"), format!("ref: refs/heads/{branch}\n")).expect("write HEAD");
    fs::write(
        git_dir.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n",
    )
    .expect("write config");

    (bin_dir, home_dir, repo_dir)
}

/// `.git` のない空のワーキングディレクトリを生成する（git リポジトリ外シナリオ用）。
pub(crate) fn setup_without_git() -> (TempDir, TempDir, TempDir) {
    let bin_dir = tempdir().expect("failed to create bin_dir");
    let home_dir = tempdir().expect("failed to create home_dir");
    let repo_dir = tempdir().expect("failed to create repo_dir");
    (bin_dir, home_dir, repo_dir)
}
