use crate::contexts::merge_readiness::application::prompt::RepoIdPort;

pub fn run(repo_id_port: &impl RepoIdPort, query: impl Fn(&str) -> Option<String>) {
    let Some(id) = repo_id_port.get() else { return };
    run_with_writer(id, query, &mut std::io::stdout());
}

fn run_with_writer(id: String, query: impl Fn(&str) -> Option<String>, w: &mut impl std::io::Write) {
    match query(&id) {
        Some(s) if !s.is_empty() => write!(w, "{s}").unwrap(),
        Some(_) => {}
        None => write!(w, "? loading").unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedRepoId(Option<String>);
    impl RepoIdPort for FixedRepoId {
        fn get(&self) -> Option<String> {
            self.0.clone()
        }
    }

    fn run_capture(id: &str, query: impl Fn(&str) -> Option<String>) -> String {
        let mut buf = Vec::new();
        run_with_writer(id.to_owned(), query, &mut buf);
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn miss_shows_loading() {
        let out = run_capture("id", |_| None);
        assert_eq!(out, "? loading");
    }

    #[test]
    fn cached_empty_shows_nothing() {
        // PRなし = キャッシュ済みの空文字列 → 何も表示しない
        let out = run_capture("id", |_| Some(String::new()));
        assert_eq!(out, "");
    }

    #[test]
    fn cached_output_shows_output() {
        let out = run_capture("id", |_| Some("✓ merge-ready".into()));
        assert_eq!(out, "✓ merge-ready");
    }
}
