use std::fs;
use std::path::{Path, PathBuf};

/// `cwd` 文字列から `repo_id` とブランチ名を一度のディレクトリ探索で導出する。
///
/// `.git` ディレクトリを上方向に探し、toplevel パス + ブランチ名を FNV-1a でハッシュ化した
/// `repo_id` と、そのブランチ名を返す。取得失敗時は `None` を返す。
pub fn repo_info_from_cwd(cwd: &str) -> Option<(String, String)> {
    let start = Path::new(cwd);
    let (toplevel, git_dir) = find_git_dir(start)?;
    let branch = read_head(&git_dir).unwrap_or_default();
    let repo_id = path_to_id(&format!("{}\0{}", toplevel.display(), branch));
    Some((repo_id, branch))
}

/// `cwd` 文字列から `repo_id` を導出する。
///
/// ブランチ名が不要な場合に使う薄いラッパー。
pub fn repo_id_from_cwd(cwd: &str) -> Option<String> {
    repo_info_from_cwd(cwd).map(|(id, _)| id)
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
