use super::command::CommandRunner;
use super::schema::{GhCompare, GhRepoView};

/// GitHub Compare API でベースブランチとの差分コミット数を取得する。
///
/// `base_ref` / `head_ref` が空の場合は `Some(0)` を返す（追跡不要）。
/// 失敗した場合は `None` を返す（呼び出し元が `SyncUnknown` として扱う）。
pub(super) fn fetch_behind_by(
    base_ref: &str,
    head_ref: &str,
    runner: &dyn CommandRunner,
) -> Option<u64> {
    if base_ref.is_empty() || head_ref.is_empty() {
        return Some(0);
    }

    let name_with_owner = match runner.run(&["repo", "view", "--json", "nameWithOwner"]) {
        Ok(bytes) => match serde_json::from_slice::<GhRepoView>(&bytes) {
            Ok(r) => r.name_with_owner,
            Err(_) => return None,
        },
        Err(_) => return None,
    };

    let path = format!("repos/{name_with_owner}/compare/{base_ref}...{head_ref}");

    match runner.run(&["api", &path]) {
        Ok(bytes) => serde_json::from_slice::<GhCompare>(&bytes)
            .map(|c| c.behind_by)
            .ok(),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::error::GhError;
    use super::*;

    struct StubRunner(fn(&[&str]) -> Result<Vec<u8>, GhError>);
    impl CommandRunner for StubRunner {
        fn run(&self, args: &[&str]) -> Result<Vec<u8>, GhError> {
            (self.0)(args)
        }
    }

    #[test]
    fn fetch_behind_by_returns_some_zero_when_base_ref_empty() {
        let runner = StubRunner(|_| panic!("should not be called"));
        assert_eq!(fetch_behind_by("", "feat/foo", &runner), Some(0));
    }

    #[test]
    fn fetch_behind_by_returns_some_zero_when_head_ref_empty() {
        let runner = StubRunner(|_| panic!("should not be called"));
        assert_eq!(fetch_behind_by("main", "", &runner), Some(0));
    }

    #[test]
    fn fetch_behind_by_returns_none_on_repo_view_command_failure() {
        let runner = StubRunner(|_| Err(GhError::ApiError("network error".to_owned())));
        assert_eq!(fetch_behind_by("main", "feat/foo", &runner), None);
    }

    #[test]
    fn fetch_behind_by_returns_none_on_repo_view_parse_failure() {
        let runner = StubRunner(|_| Ok(b"not-json".to_vec()));
        assert_eq!(fetch_behind_by("main", "feat/foo", &runner), None);
    }

    #[test]
    fn fetch_behind_by_returns_none_on_compare_command_failure() {
        let runner = StubRunner(|args| {
            if args.contains(&"nameWithOwner") {
                Ok(br#"{"nameWithOwner":"owner/repo"}"#.to_vec())
            } else {
                Err(GhError::ApiError("compare failed".to_owned()))
            }
        });
        assert_eq!(fetch_behind_by("main", "feat/foo", &runner), None);
    }

    #[test]
    fn fetch_behind_by_returns_none_on_compare_parse_failure() {
        let runner = StubRunner(|args| {
            if args.contains(&"nameWithOwner") {
                Ok(br#"{"nameWithOwner":"owner/repo"}"#.to_vec())
            } else {
                Ok(b"not-json".to_vec())
            }
        });
        assert_eq!(fetch_behind_by("main", "feat/foo", &runner), None);
    }

    #[test]
    fn fetch_behind_by_returns_behind_by_on_success() {
        let runner = StubRunner(|args| {
            if args.contains(&"nameWithOwner") {
                Ok(br#"{"nameWithOwner":"owner/repo"}"#.to_vec())
            } else {
                Ok(br#"{"behind_by":3}"#.to_vec())
            }
        });
        assert_eq!(fetch_behind_by("main", "feat/foo", &runner), Some(3));
    }
}
