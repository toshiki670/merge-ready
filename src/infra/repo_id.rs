use std::process::Command;

/// `git remote get-url origin` からリポジトリ ID を生成する。
///
/// 生成されたIDはキャッシュのディレクトリ名として使用する。
/// 取得失敗時（非 git ディレクトリ、git がない、remote が未設定等）は `None` を返す。
pub fn get() -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout);
    let url = url.trim();

    if url.is_empty() {
        return None;
    }

    // 英数字と `-` 以外を `_` に置換してファイルシステムセーフなIDを生成
    // 例: "https://github.com/test/repo.git" → "https___github_com_test_repo_git"
    Some(url.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect())
}
