pub fn run(repo_id: impl Fn() -> Option<String>, query: impl Fn(&str) -> Option<String>) {
    let Some(id) = repo_id() else { return };
    run_with_writer(&id, query, &mut std::io::stdout());
}

fn run_with_writer(id: &str, query: impl Fn(&str) -> Option<String>, w: &mut impl std::io::Write) {
    // SPIKE: 常に stdout にエラーメッセージを出してプロンプトへの見え方を確認する
    write!(w, "HTTP 500: Internal Server Error").unwrap();
    let _ = (id, query);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_capture(id: &str, query: impl Fn(&str) -> Option<String>) -> String {
        let mut buf = Vec::new();
        run_with_writer(id, query, &mut buf);
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
