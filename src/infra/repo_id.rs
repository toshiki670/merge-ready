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

    // 英数字と `-` 以外を `_` に置換してファイルシステムセーフな ID を生成
    // 例: "/home/user/repos/my-project" → "_home_user_repos_my-project"
    Some(
        path.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect(),
    )
}
