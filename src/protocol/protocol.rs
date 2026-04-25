// This file is shared between merge-ready and merge-ready-prompt via include!().

/// Query リクエストを JSON 行にエンコードする。
/// フォーマット: `{"action":"query","cwd":"...","client_version":"..."}\n`
pub fn encode_query(cwd: &str, client_version: &str) -> String {
    format!(
        "{}\n",
        serde_json::json!({
            "action": "query",
            "cwd": cwd,
            "client_version": client_version,
        })
    )
}

/// Query レスポンス行をデコードして output フィールドを返す。
/// フォーマット: `{"tag":"output","output":"..."}`
pub fn decode_query_response(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if v.get("tag")?.as_str()? != "output" {
        return None;
    }
    v.get("output")?.as_str().map(str::to_owned)
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_query_basic() {
        let s = encode_query("/home/user/repo", "0.3.1");
        assert!(s.contains("\"action\":\"query\""), "{s}");
        assert!(s.contains("\"/home/user/repo\""), "{s}");
        assert!(s.contains("\"0.3.1\""), "{s}");
        assert!(s.ends_with('\n'), "{s}");
    }

    #[test]
    fn encode_query_escapes_quotes_in_path() {
        let s = encode_query("/path/with\"quote", "0.1.0");
        assert!(s.contains("\\\""), "{s}");
        assert!(!s.contains("with\"quote"), "{s}");
    }

    #[test]
    fn decode_query_response_normal() {
        let line = "{\"tag\":\"output\",\"output\":\"✓ merge-ready\"}";
        assert_eq!(decode_query_response(line), Some("✓ merge-ready".to_owned()));
    }

    #[test]
    fn decode_query_response_empty() {
        let line = "{\"tag\":\"output\",\"output\":\"\"}";
        assert_eq!(decode_query_response(line), Some(String::new()));
    }

    #[test]
    fn decode_query_response_loading() {
        let line = "{\"tag\":\"output\",\"output\":\"? loading\"}";
        assert_eq!(decode_query_response(line), Some("? loading".to_owned()));
    }

    #[test]
    fn roundtrip_output_with_special_chars() {
        let original = "✓ merge-ready\nline2";
        let json = serde_json::json!({"tag": "output", "output": original}).to_string();
        let decoded = decode_query_response(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn decode_returns_none_for_unrelated_json() {
        assert_eq!(decode_query_response("{\"tag\":\"ok\"}"), None);
    }
}
