use std::fs;
use std::path::{Path, PathBuf};

/// `.git` ディレクトリを直接読み取ってキャッシュキーを生成する。
///
/// worktree パスとブランチ名の組み合わせをハッシュ化することで、
/// 同一 worktree でのブランチ切り替えや複数 worktree の並走でも衝突しない。
/// 取得失敗時（非 git ディレクトリ等）は `None` を返す。
pub fn get() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let (toplevel, git_dir) = find_git_dir(&cwd)?;
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
            // worktree / サブモジュール: "gitdir: /path/to/.git/worktrees/xxx"
            let content = fs::read_to_string(&dot_git).ok()?;
            let real = content.strip_prefix("gitdir: ")?.trim();
            return Some((dir, PathBuf::from(real)));
        }
        if !dir.pop() {
            return None; // git リポジトリ外
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
///
/// 文字置換ではなくハッシュを使うことで、`/` と `_` が同じ `_` に潰れる衝突を回避する。
/// 例: `"/home/user/repos/my_project"` と `"/home/user/repos/my/project"` は別の ID になる。
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

    // worktree: main_repo/.git/worktrees/feat/HEAD + worktree/.git (file)
    fn make_worktree(branch: &str) -> (TempDir, TempDir) {
        let main_repo = TempDir::new().unwrap();
        let worktree_meta = main_repo.path().join(".git/worktrees/feat");
        fs::create_dir_all(&worktree_meta).unwrap();
        fs::write(worktree_meta.join("HEAD"), format!("ref: refs/heads/{branch}\n")).unwrap();

        let worktree = TempDir::new().unwrap();
        fs::write(
            worktree.path().join(".git"),
            format!("gitdir: {}\n", worktree_meta.display()),
        )
        .unwrap();
        (main_repo, worktree)
    }

    // ── find_git_dir ──────────────────────────────────────────────────────────

    #[test]
    fn normal_repo_finds_toplevel_and_git_dir() {
        let repo = make_normal_repo("main");
        let (toplevel, git_dir) = find_git_dir(repo.path()).unwrap();
        assert_eq!(toplevel, repo.path());
        assert_eq!(git_dir, repo.path().join(".git"));
    }

    #[test]
    fn subdirectory_finds_repo_root() {
        let repo = make_normal_repo("main");
        let subdir = repo.path().join("a/b/c");
        fs::create_dir_all(&subdir).unwrap();
        let (toplevel, _) = find_git_dir(&subdir).unwrap();
        assert_eq!(toplevel, repo.path());
    }

    #[test]
    fn worktree_finds_toplevel_and_real_git_dir() {
        let (main_repo, worktree) = make_worktree("feat");
        let (toplevel, git_dir) = find_git_dir(worktree.path()).unwrap();
        assert_eq!(toplevel, worktree.path());
        assert_eq!(git_dir, main_repo.path().join(".git/worktrees/feat"));
    }

    #[test]
    fn outside_repo_returns_none() {
        let dir = TempDir::new().unwrap(); // .git のない空ディレクトリ
        assert!(find_git_dir(dir.path()).is_none());
    }

    // ── read_head ─────────────────────────────────────────────────────────────

    #[test]
    fn read_head_returns_branch_name() {
        let repo = make_normal_repo("main");
        let branch = read_head(&repo.path().join(".git")).unwrap();
        assert_eq!(branch, "main");
    }

    #[test]
    fn detached_head_returns_none() {
        let dir = TempDir::new().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), "abc123deadbeef\n").unwrap();
        assert!(read_head(&git_dir).is_none());
    }

    #[test]
    fn worktree_head_returns_branch_name() {
        let (main_repo, _worktree) = make_worktree("feat");
        let git_dir = main_repo.path().join(".git/worktrees/feat");
        let branch = read_head(&git_dir).unwrap();
        assert_eq!(branch, "feat");
    }

    // ── path_to_id ────────────────────────────────────────────────────────────

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
