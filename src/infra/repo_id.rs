use std::process::Command;

/// `git rev-parse --show-toplevel` からワークツリーの ID を生成する。
///
/// 生成された ID はキャッシュのファイル名として使用する。
/// worktree ごとに固有のパスが返るため、同一リモートの worktree 間で衝突しない。
/// 取得失敗時（非 git ディレクトリ、git がない等）は `None` を返す。
pub fn get() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&output.stdout);
    let path = path.trim();

    if path.is_empty() {
        return None;
    }

    Some(path_to_id(path))
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
