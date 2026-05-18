use serde::{Deserialize, Serialize};

use super::refresh_mode::RefreshMode;

/// PR 単体のレンダリング済み出力。watch 表示用かつ IPC ワイヤフォーマット上の表現。
#[derive(Debug, Serialize, Deserialize)]
pub struct PrOutput {
    pub pr_id: u64,
    pub output: String,
}

/// キャッシュエントリの表示用 DTO。
#[derive(Debug, Serialize, Deserialize)]
pub struct EntryDto {
    pub cwd: String,
    pub branch: String,
    #[serde(default)]
    pub pr_id: Option<u64>,
    pub output: String,
    pub cached_at_secs: u64,
}

/// デーモンへ送信するリクエスト
#[derive(Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Request {
    /// `merge-ready-prompt` から送られるクエリ。`cwd` から daemon が `repo_id` を導出する。
    Query {
        cwd: String,
    },
    /// バックグラウンドワーカーがキャッシュを更新するときに送るリクエスト。
    /// `gh` CLI で取得した PR 評価結果（`output`）を daemon のインメモリキャッシュに書き込む。
    Update {
        repo_id: String,
        output: String,
        refresh_mode: RefreshMode,
        pr_outputs: Vec<PrOutput>,
    },
    Stop,
    Status,
    Entries,
}

/// デーモンから返却されるレスポンス
#[derive(Serialize, Deserialize)]
#[serde(tag = "tag", rename_all = "snake_case")]
pub enum Response {
    /// Query に対する応答。Fresh/Stale/Miss をすべて output 文字列に統合する。
    /// Miss または初回ロード中は "? loading"、PR なしは ""。
    Output {
        output: String,
    },
    Ok,
    Status {
        entries: usize,
        uptime_secs: u64,
        version: String,
    },
    Entries {
        entries: Vec<EntryDto>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_dto_serializes() {
        let dto = EntryDto {
            cwd: "/tmp".into(),
            branch: "main".into(),
            pr_id: Some(123),
            output: "✓ Ready".into(),
            cached_at_secs: 1_000,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("cached_at_secs"));
        assert!(json.contains("pr_id"));
        assert!(json.contains("/tmp"));
    }

    #[test]
    fn refresh_mode_round_trip() {
        for mode in [RefreshMode::Hot, RefreshMode::Warm, RefreshMode::Terminal] {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: RefreshMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, parsed);
        }
    }
}
