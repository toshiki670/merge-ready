use serde::{Deserialize, Serialize};

/// デーモンへ送信するリクエスト
#[derive(Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Request {
    /// `merge-ready-prompt` から送られるクエリ。`cwd` から daemon が `repo_id` を導出する。
    Query {
        cwd: String,
        client_version: String,
    },
    /// バックグラウンドワーカーがキャッシュを更新するときに送るリクエスト。
    /// `gh` CLI で取得した PR 評価結果（`output`）を daemon のインメモリキャッシュに書き込む。
    Update {
        repo_id: String,
        output: String,
        /// PR が closed / merged 状態かどうか。
        is_terminal: bool,
    },
    Stop,
    Status,
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
        pid: u32,
        entries: usize,
        uptime_secs: u64,
        version: String,
    },
}
