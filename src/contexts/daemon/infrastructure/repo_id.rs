use std::fs;
use std::path::{Path, PathBuf};

/// `cwd` 文字列から `repo_id` を導出する。
///
/// `.git` ディレクトリを上方向に探し、toplevel パス + ブランチ名を FNV-1a でハッシュ化する。
/// 取得失敗時は `None` を返す。
pub fn repo_id_from_cwd(cwd: &str) -> Option<String> {
    let start = Path::new(cwd);
    let (toplevel, git_dir) = find_git_dir(start)?;
    let branch = read_head(&git_dir).unwrap_or_default();
    Some(path_to_id(&format!("{}\0{}", toplevel.display(), branch)))
}

/// カレントディレクトリから上に向かって `.git` を探す。
///
/// worktree またはサブモジュールの場合 `.git` はファイル（`"gitdir: <path>"` 形式）。
fn find_git_dir(start: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut dir = start.to_path_buf();
    loop {
        let dot_git = dir.join(".git");
        if dot_git.is_dir() {
            return Some((dir, dot_git));
        }
        if dot_git.is_file() {
            let content = fs::read_to_string(&dot_git).ok()?;
            let real = content.strip_prefix("gitdir: ")?.trim();
            return Some((dir, PathBuf::from(real)));
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// `.git/HEAD` から `"ref: refs/heads/main"` → `"main"` を取り出す。
///
/// detached HEAD は `None` を返す（`unwrap_or_default()` で `""` になる）。
fn read_head(git_dir: &Path) -> Option<String> {
    let content = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    Some(content.strip_prefix("ref: refs/heads/")?.trim().to_owned())
}

/// パス文字列を FNV-1a ハッシュでファイルシステムセーフな ID に変換する。
#[must_use]
pub fn path_to_id(path: &str) -> String {
    let mut hash: u64 = 14_695_981_039_346_656_037;
    for byte in path.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_normal_repo(branch: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), format!("ref: refs/heads/{branch}\n")).unwrap();
        dir
    }

    fn make_worktree(branch: &str) -> (TempDir, TempDir) {
        let main_repo = TempDir::new().unwrap();
        let worktree_meta = main_repo.path().join(format!(".git/worktrees/{branch}"));
        fs::create_dir_all(&worktree_meta).unwrap();
        fs::write(
            worktree_meta.join("HEAD"),
            format!("ref: refs/heads/{branch}\n"),
        )
        .unwrap();

        let worktree = TempDir::new().unwrap();
        fs::write(
            worktree.path().join(".git"),
            format!("gitdir: {}\n", worktree_meta.display()),
        )
        .unwrap();
        (main_repo, worktree)
    }

    #[test]
    fn normal_repo_finds_repo_id() {
        let repo = make_normal_repo("main");
        let id = repo_id_from_cwd(repo.path().to_str().unwrap());
        assert!(id.is_some());
    }

    #[test]
    fn outside_repo_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(repo_id_from_cwd(dir.path().to_str().unwrap()).is_none());
    }

    #[test]
    fn worktree_repo_id_differs_from_main() {
        let main_repo = make_normal_repo("main");
        let (_main2, worktree) = make_worktree("feat");

        let main_id = repo_id_from_cwd(main_repo.path().to_str().unwrap()).unwrap();
        let wt_id = repo_id_from_cwd(worktree.path().to_str().unwrap()).unwrap();
        assert_ne!(main_id, wt_id);
    }

    #[test]
    fn different_paths_produce_different_ids() {
        let a = path_to_id("/home/user/repos/my_project\0main");
        let b = path_to_id("/home/user/repos/my/project\0main");
        assert_ne!(a, b);
    }

    #[test]
    fn same_input_produces_same_id() {
        let a = path_to_id("/repo\0main");
        let b = path_to_id("/repo\0main");
        assert_eq!(a, b);
    }
}
