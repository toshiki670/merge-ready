use serde::{Deserialize, Serialize};

use crate::contexts::daemon::domain::cache::RefreshMode;

impl Serialize for RefreshMode {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            RefreshMode::Hot => s.serialize_str("hot"),
            RefreshMode::Warm => s.serialize_str("warm"),
            RefreshMode::Terminal => s.serialize_str("terminal"),
        }
    }
}

impl<'de> Deserialize<'de> for RefreshMode {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            "hot" => Ok(RefreshMode::Hot),
            "warm" => Ok(RefreshMode::Warm),
            "terminal" => Ok(RefreshMode::Terminal),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &["hot", "warm", "terminal"],
            )),
        }
    }
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
        refresh_mode: RefreshMode,
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
