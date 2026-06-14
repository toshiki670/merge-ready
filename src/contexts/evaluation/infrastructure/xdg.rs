//! XDG Base Directory 仕様に基づくベースディレクトリ解決。

use std::ffi::OsString;
use std::path::PathBuf;

/// 環境変数 `var` を XDG ベースディレクトリとして解決する。
///
/// XDG Base Directory 仕様では各環境変数は**絶対パス**でなければならず、
/// 未設定・空文字・相対パスはすべて無効とみなす。無効な場合は `None` を返し、
/// 呼び出し側でデフォルト（`$HOME/...`）へフォールバックさせる。
pub(crate) fn base_dir(var: &str) -> Option<PathBuf> {
    resolve(std::env::var_os(var))
}

/// 解決ロジック本体。グローバルな環境に触れない純粋関数なので、相対パス・空文字の
/// 扱いを単体テストで固定できる（E2E では cwd 依存になり安定検証が難しい）。
fn resolve(value: Option<OsString>) -> Option<PathBuf> {
    let path = PathBuf::from(value?);
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_is_none() {
        assert_eq!(resolve(None), None);
    }

    #[test]
    fn empty_is_ignored() {
        assert_eq!(resolve(Some(OsString::from(""))), None);
    }

    #[test]
    fn relative_is_ignored() {
        assert_eq!(resolve(Some(OsString::from("relative/dir"))), None);
    }

    #[test]
    fn absolute_is_used() {
        assert_eq!(
            resolve(Some(OsString::from("/abs/dir"))),
            Some(PathBuf::from("/abs/dir"))
        );
    }
}
