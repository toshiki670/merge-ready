use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;

use super::daemon_server::RefreshFn;
use super::daemon_state_actor::DaemonStateHandle;
use super::paths::Paths;
use super::request_handler::ActionResult;
use super::restart;
use crate::shared::protocol::Request;

/// 1 リクエスト行として受理する最大バイト数。
///
/// 信頼境界は同一ユーザだが defense-in-depth として、改行まで無制限に読み取る
/// ことで悪意あるローカルプロセスがメモリを枯渇させるのを防ぐ。最大の正当な
/// リクエストは `Update`（PR ごとのレンダリング済み出力を多数含む）なので、
/// 通常のステータス文字列に対して十分余裕のある 1 MiB を上限とする。
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

pub(super) async fn handle(
    mut stream: UnixStream,
    state: &DaemonStateHandle,
    on_refresh: RefreshFn,
    exit_tx: &UnboundedSender<()>,
    paths: &Paths,
    handle: &Handle,
) {
    let buf = {
        let mut reader = BufReader::new(&mut stream);
        match read_request_line(&mut reader).await {
            Some(buf) => buf,
            None => return,
        }
    };

    let request: Request = match serde_json::from_str(buf.trim()) {
        Ok(r) => r,
        Err(_) => return,
    };

    let Some(ActionResult {
        response,
        refresh_repo_id,
        refresh_cwd,
        stop,
    }) = state.process(request).await
    else {
        return;
    };

    if let Ok(json) = serde_json::to_string(&response) {
        let _ = stream.write_all(format!("{json}\n").as_bytes()).await;
    }
    drop(stream);

    if let (Some(repo_id), Some(cwd)) = (refresh_repo_id, refresh_cwd) {
        super::daemon_server::spawn_refresh(&repo_id, &cwd, on_refresh, handle);
    }

    if stop {
        restart::cleanup(paths);
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = exit_tx.send(());
    }
}

/// ソケットから 1 リクエスト行を読み取る。
///
/// `take` で [`MAX_REQUEST_BYTES`] の上限を設けるため、改行が来ない / 巨大な行
/// でも無制限にメモリを確保しない。空入力 / 読み取りエラー時は `None` を返す。
/// 上限に達した場合は改行なしで打ち切られた文字列を返すため、呼び出し側の
/// JSON パースが失敗して安全に拒否される。
async fn read_request_line<R>(reader: &mut R) -> Option<String>
where
    R: AsyncBufReadExt + Unpin,
{
    let mut buf = String::new();
    let mut limited = reader.take(MAX_REQUEST_BYTES);
    if limited.read_line(&mut buf).await.is_err() || buf.is_empty() {
        return None;
    }
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_a_full_line_within_the_limit() {
        let input = b"hello world\n";
        let mut reader = BufReader::new(&input[..]);
        let line = read_request_line(&mut reader).await;
        assert_eq!(line.as_deref(), Some("hello world\n"));
    }

    #[tokio::test]
    async fn returns_none_for_empty_input() {
        let input: &[u8] = b"";
        let mut reader = BufReader::new(input);
        assert!(read_request_line(&mut reader).await.is_none());
    }

    #[tokio::test]
    async fn truncates_input_exceeding_the_byte_limit() {
        let limit = usize::try_from(MAX_REQUEST_BYTES).expect("limit fits in usize");
        // 改行のない巨大入力。`take` がなければ read_line は EOF まで全量を確保する。
        let oversized = vec![b'a'; limit + 1024];
        let mut reader = BufReader::new(&oversized[..]);
        let line = read_request_line(&mut reader)
            .await
            .expect("should read up to the limit");
        // 上限ちょうどで打ち切られ、超過分は読み込まれない。
        assert_eq!(line.len(), limit);
        // 改行が含まれないため、呼び出し側の JSON パースは失敗する。
        assert!(!line.contains('\n'));
    }
}
