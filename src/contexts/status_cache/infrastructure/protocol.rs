use serde::{Deserialize, Serialize};

/// デーモンへ送信するリクエスト
#[derive(Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Request {
    Query { repo_id: String },
    Update { repo_id: String, output: String },
    Stop,
    Status,
}

/// デーモンから返却されるレスポンス
#[derive(Serialize, Deserialize)]
#[serde(tag = "tag", rename_all = "snake_case")]
pub enum Response {
    Fresh {
        output: String,
    },
    Stale {
        output: String,
    },
    Miss,
    Ok,
    Status {
        pid: u32,
        entries: usize,
        uptime_secs: u64,
    },
}
