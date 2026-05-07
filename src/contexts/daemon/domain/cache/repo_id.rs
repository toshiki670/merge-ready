/// リポジトリとブランチの組み合わせを識別する値オブジェクト。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoId(String);

impl RepoId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RepoId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<RepoId> for String {
    fn from(r: RepoId) -> Self {
        r.0
    }
}

impl std::fmt::Display for RepoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_id_as_str_returns_inner() {
        let id = RepoId::new("abc123");
        assert_eq!(id.as_str(), "abc123");
    }

    #[test]
    fn repo_id_equality() {
        assert_eq!(RepoId::new("a"), RepoId::new("a"));
        assert_ne!(RepoId::new("a"), RepoId::new("b"));
    }

    #[test]
    fn repo_id_from_and_into_string() {
        let id = RepoId::from("test".to_owned());
        assert_eq!(id.as_str(), "test");
        let s: String = RepoId::new("test").into();
        assert_eq!(s, "test");
    }
}
