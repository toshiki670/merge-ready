use std::process::Command;

/// `git rev-parse --show-toplevel` と `git branch --show-current` からキャッシュキーを生成する。
///
/// worktree パスとブランチ名の組み合わせをハッシュ化することで、
/// 同一 worktree でのブランチ切り替えや複数 worktree の並走でも衝突しない。
/// 取得失敗時（非 git ディレクトリ、git がない等）は `None` を返す。
pub fn get() -> Option<String> {
    let toplevel = run_git(&["rev-parse", "--show-toplevel"])?;
    // detached HEAD の場合は空文字列になるが、それでもキーとして一意に扱う
    let branch = run_git(&["branch", "--show-current"]).unwrap_or_default();
    Some(path_to_id(&format!("{toplevel}\0{branch}")))
}

fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let s = String::from_utf8_lossy(&output.stdout);
    let s = s.trim();

    if s.is_empty() {
        return None;
    }

    Some(s.to_owned())
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
