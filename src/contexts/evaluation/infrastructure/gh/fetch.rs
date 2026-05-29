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

    let path = build_compare_path(base_ref, head_ref);

    match run_gh(&["api", &path], cwd).await {
        Ok(bytes) => serde_json::from_slice::<GhCompare>(&bytes)
            .map(|c| c.behind_by)
            .ok(),
        Err(_) => None,
    }
}

/// Compare API のリクエストパスを組み立てる。
///
/// `owner`/`repo` は gh が cwd のリポジトリ（または `GH_REPO`）から補完する
/// placeholder。`base_ref` / `head_ref` は GitHub 返却のブランチ名だが、
/// defense-in-depth として各 ref をパスセグメントとして URL エンコードする。
/// `...` 区切りはそのまま残す。
fn build_compare_path(base_ref: &str, head_ref: &str) -> String {
    let base = encode_path_segment(base_ref);
    let head = encode_path_segment(head_ref);
    format!("repos/{{owner}}/{{repo}}/compare/{base}...{head}")
}

/// 文字列を 1 つの URL パスセグメントとしてパーセントエンコードする。
///
/// RFC 3986 の `unreserved`（`ALPHA` / `DIGIT` / `-._~`）以外をエンコードする。
/// `/` を含むあらゆる予約文字・非 ASCII をエスケープするため、ref 名が
/// パス区切りや別セグメントへ漏れ出すことはない。
fn encode_path_segment(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        if is_unreserved(byte) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(hex_upper(byte >> 4));
            out.push(hex_upper(byte & 0x0f));
        }
    }
    out
}

/// RFC 3986 の `unreserved` 文字か判定する。
fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

/// 下位 4 ビットを大文字 16 進数 1 桁へ変換する。
fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + (nibble - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_plain_refs_unchanged() {
        assert_eq!(
            build_compare_path("main", "feature"),
            "repos/{owner}/{repo}/compare/main...feature"
        );
    }

    #[test]
    fn keeps_unreserved_characters_unchanged() {
        // `-._~` と英数字は unreserved なのでエンコードされない。
        assert_eq!(
            build_compare_path("release-1.0_x~", "dev-2.0_y~"),
            "repos/{owner}/{repo}/compare/release-1.0_x~...dev-2.0_y~"
        );
    }

    #[test]
    fn encodes_slash_in_ref_so_it_does_not_become_a_path_separator() {
        // `feature/foo` の `/` がパス区切りに漏れないことを確認する。
        assert_eq!(
            build_compare_path("feature/foo", "main"),
            "repos/{owner}/{repo}/compare/feature%2Ffoo...main"
        );
    }

    #[test]
    fn encodes_reserved_and_unsafe_characters() {
        // 空白・`#`・`?`・`%` などの予約／危険文字をエスケープする。
        assert_eq!(
            build_compare_path("a b#c?d%e", "x&y=z"),
            "repos/{owner}/{repo}/compare/a%20b%23c%3Fd%25e...x%26y%3Dz"
        );
    }

    #[test]
    fn does_not_escape_the_triple_dot_separator() {
        // ref 自体に含まれる `.` は unreserved なので、区切りの `...` と
        // 区別なくそのまま残る（区切り構造は維持される）。
        assert_eq!(
            build_compare_path("v1.2.3", "v1.3.0"),
            "repos/{owner}/{repo}/compare/v1.2.3...v1.3.0"
        );
    }

    #[test]
    fn encodes_non_ascii_as_utf8_percent_octets() {
        // 非 ASCII（日本語）は UTF-8 バイト列をパーセントエンコードする。
        assert_eq!(
            build_compare_path("機能", "main"),
            "repos/{owner}/{repo}/compare/%E6%A9%9F%E8%83%BD...main"
        );
    }
}
