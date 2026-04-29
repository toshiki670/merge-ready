use serde::{Deserialize, Serialize};

/// IPC ワイヤフォーマット上のリフレッシュモード DTO。
/// Domain 層の `RefreshMode` とは独立して定義する。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshModeDto {
    Hot,
    Warm,
    Terminal,
}

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
        refresh_mode: RefreshModeDto,
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
        entries: usize,
        uptime_secs: u64,
        version: String,
    },
}
