use std::path::Path;

use super::schema::GhCompare;
use crate::shared::process_gh::run_gh;

/// GitHub Compare API でベースブランチとの差分コミット数を取得する。
///
/// `base_ref` / `head_ref` が空の場合は `Some(0)` を返す（追跡不要）。
/// 失敗した場合は `None` を返す（呼び出し元が `SyncUnknown` として扱う）。
pub(super) async fn fetch_behind_by(
    base_ref: &str,
    head_ref: &str,
    cwd: Option<&Path>,
) -> Option<u64> {
    if base_ref.is_empty() || head_ref.is_empty() {
        return Some(0);
    }

    // owner/repo は gh が cwd のリポジトリ（または GH_REPO）から補完する placeholder。
    // PR ごとの `gh repo view --json nameWithOwner` を撤廃するために使う。
    let path = format!("repos/{{owner}}/{{repo}}/compare/{base_ref}...{head_ref}");

    match run_gh(&["api", &path], cwd).await {
        Ok(bytes) => serde_json::from_slice::<GhCompare>(&bytes)
            .map(|c| c.behind_by)
            .ok(),
        Err(_) => None,
    }
}
