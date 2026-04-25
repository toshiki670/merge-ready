// IMPORTANT: This file is shared between merge-ready and merge-ready-prompt via include!().
// Only std is allowed here. No external crate imports.

/// Query リクエストを JSON 行にエンコードする。
/// フォーマット: `{"action":"query","cwd":"...","client_version":"..."}\n`
pub fn encode_query(cwd: &str, client_version: &str) -> String {
    format!(
        "{{\"action\":\"query\",\"cwd\":{},\"client_version\":{}}}\n",
        json_string(cwd),
        json_string(client_version)
    )
}

/// Query レスポンス行をデコードして output フィールドを返す。
/// フォーマット: `{"tag":"output","output":"..."}`
pub fn decode_query_response(line: &str) -> Option<String> {
    extract_json_string_field(line.trim(), "output")
}

// ─── internal helpers ────────────────────────────────────────────────────────

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let n = c as u32;
                out.push_str("\\u");
                out.push(hex_digit((n >> 12) & 0xf));
                out.push(hex_digit((n >> 8) & 0xf));
                out.push(hex_digit((n >> 4) & 0xf));
                out.push(hex_digit(n & 0xf));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[allow(clippy::cast_possible_truncation)]
fn hex_digit(n: u32) -> char {
    // n is always 0..=15, so truncation is safe
    if n <= 9 {
        (b'0' + n as u8) as char
    } else {
        (b'a' + (n - 10) as u8) as char
    }
}

fn json_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('/') => out.push('/'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Some(n) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(n);
                }
            }
            Some('\\') | None => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

/// JSON オブジェクトから指定キーの文字列値を抽出する最小パーサー。
fn extract_json_string_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let pos = json.find(needle.as_str())?;
    let after_key = &json[pos + needle.len()..];
    let colon_pos = after_key.find(':')?;
    let rest = after_key[colon_pos + 1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let inner = &rest[1..];
    Some(json_unescape(&collect_until_close_quote(inner)))
}

fn collect_until_close_quote(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => {
                out.push('\\');
                if let Some(e) = chars.next() {
                    out.push(e);
                    if e == 'u' {
                        for _ in 0..4 {
                            if let Some(h) = chars.next() {
                                out.push(h);
                            }
                        }
                    }
                }
            }
            c => out.push(c),
        }
    }
    out
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
        // " in the path must be escaped to \" in the JSON
        assert!(s.contains("\\\""), "{s}");
        // Original unescaped quote must not appear between cwd json delimiters
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
        let json = format!("{{\"tag\":\"output\",\"output\":{}}}", json_string(original));
        let decoded = decode_query_response(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn decode_returns_none_for_unrelated_json() {
        assert_eq!(decode_query_response("{\"tag\":\"ok\"}"), None);
    }
}
