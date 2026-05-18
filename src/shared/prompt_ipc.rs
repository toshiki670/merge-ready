//! `merge-ready-prompt` バイナリが daemon と通信するための IPC 型。
//!
//! [`Request`] でクエリを組み立てて [`Request::encode`] でワイヤ形式に変換し、
//! daemon の応答は [`Response::decode`] でパースする。

// 設計メモ（doc には出さない）:
// ここで公開する Request / Response は daemon の IPC プロトコル全体
// （Query / Update / Stop / Status / Entries など）ではなく、prompt 経路で実際に
// 使う 1 種類だけを切り出した narrow 型。内部で protocol::Request::Query /
// protocol::Response::Output に変換することで src_prompt と daemon 間のワイヤ
// フォーマット drift を構造的に防ぐ。
use super::protocol;

/// `merge-ready-prompt` から daemon へ送るクエリ。
pub struct Request {
    /// 問い合わせ対象のワーキングディレクトリ。
    pub cwd: String,
}

/// daemon からの応答。
pub struct Response {
    /// レンダリング済みのプロンプト文字列。
    pub output: String,
}

impl Request {
    /// ワイヤ形式（改行終端の JSON）にエンコードする。失敗時は空文字列を返す。
    #[must_use]
    pub fn encode(&self) -> String {
        let mut s = serde_json::to_string(&protocol::Request::Query {
            cwd: self.cwd.clone(),
        })
        .unwrap_or_default();
        s.push('\n');
        s
    }
}

impl Response {
    /// daemon からのバイト列をデコードする。形式が想定外なら `None`。
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let line = bytes.split(|&b| b == b'\n').next()?;
        match serde_json::from_slice::<protocol::Response>(line).ok()? {
            protocol::Response::Output { output } => Some(Self { output }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_produces_query_json_with_newline() {
        let req = Request {
            cwd: "/tmp/repo".to_owned(),
        };
        let wire = req.encode();
        assert!(wire.ends_with('\n'));
        assert!(wire.contains("\"action\":\"query\""));
        assert!(wire.contains("\"cwd\":\"/tmp/repo\""));
    }

    #[test]
    fn decode_extracts_output_string() {
        let line = r#"{"tag":"output","output":"✓ Ready for merge"}"#;
        let resp = Response::decode(line.as_bytes()).expect("decodes");
        assert_eq!(resp.output, "✓ Ready for merge");
    }

    #[test]
    fn decode_returns_none_for_non_output() {
        let line = br#"{"tag":"ok"}"#;
        assert!(Response::decode(line).is_none());
    }

    #[test]
    fn decode_returns_none_for_garbage() {
        assert!(Response::decode(b"not json").is_none());
    }

    #[test]
    fn round_trip_via_internal_request() {
        // prompt_ipc::Request::encode → 内部 protocol::Request としてデコードできる
        let req = Request {
            cwd: "/some/cwd".to_owned(),
        };
        let wire = req.encode();
        let parsed: protocol::Request =
            serde_json::from_str(wire.trim()).expect("parses as protocol::Request");
        match parsed {
            protocol::Request::Query { cwd } => assert_eq!(cwd, "/some/cwd"),
            _ => panic!("expected Query variant"),
        }
    }
}
