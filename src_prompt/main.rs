// merge-ready-prompt: 軽量なシェルプロンプト用バイナリ。

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::time::{Duration, Instant};

use merge_ready::{prompt_ipc, prompt_socket_path};

const RESPONSE_TIMEOUT_MS: u64 = 500;

fn main() {
    let output = query_daemon().unwrap_or_else(|| {
        // 接続失敗 → daemon を非同期起動して "? loading" を返す
        spawn_daemon();
        "? loading".to_owned()
    });
    // 信頼境界は同一ユーザだが defense-in-depth として、端末へ出力する前に
    // 危険な制御文字を除去する。正当な ANSI カラー（SGR）は保持する。
    print!("{}", sanitize_output(&output));
}

/// daemon から受け取った出力を端末へ表示する前にサニタイズする。
///
/// プロンプトは 1 行のステータス表示用途であり、表示に必要な ANSI カラー
/// （SGR: `ESC [ ... m`）は保持する。それ以外の制御文字はすべて除去する:
///
/// - C0 制御文字（`\x00`–`\x1f`）と DEL（`\x7f`）。改行・タブも含めて除去し、
///   悪意ある `Update` による複数行注入やプロンプト破壊を防ぐ。
/// - C1 制御文字（`U+0080`–`U+009F`）。
/// - SGR 以外の ANSI エスケープシーケンス（カーソル移動・画面消去・OSC など）。
///
/// SGR シーケンスは `ESC [` で始まり、パラメータ（`0x30`–`0x3f`）と中間
/// バイト（`0x20`–`0x2f`）が続き、終端文字 `m`（`0x6d`）で終わる。終端が `m`
/// 以外の CSI シーケンスや、`ESC [` 以外で始まるエスケープは丸ごと除去する。
#[must_use]
fn sanitize_output(input: &str) -> String {
    const ESC: char = '\u{1b}';
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == ESC {
            // `ESC [` で始まり `m` で終わる SGR（カラー）シーケンスだけを保持する。
            if matches!(chars.peek(), Some('[')) {
                let mut seq = String::from(ESC);
                seq.push(chars.next().expect("peeked '['"));
                let mut terminated_with_m = false;
                for sc in chars.by_ref() {
                    seq.push(sc);
                    // CSI シーケンスの終端は 0x40–0x7e。終端まで読み切る。
                    if ('\u{40}'..='\u{7e}').contains(&sc) {
                        terminated_with_m = sc == 'm';
                        break;
                    }
                }
                if terminated_with_m {
                    out.push_str(&seq);
                }
                // SGR 以外の CSI は破棄する（seq は out へ push しない）。
            }
            // `ESC [` 以外のエスケープ（OSC など）は ESC を捨てて続行する。
            continue;
        }

        if is_disallowed_control(c) {
            continue;
        }
        out.push(c);
    }

    out
}

/// 表示を許可しない制御文字かどうか。
///
/// C0（`\x00`–`\x1f`、改行・タブを含む）、DEL（`\x7f`）、C1（`U+0080`–`U+009F`）。
fn is_disallowed_control(c: char) -> bool {
    matches!(c, '\u{00}'..='\u{1f}' | '\u{7f}'..='\u{9f}')
}

fn query_daemon() -> Option<String> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut stream = UnixStream::connect(prompt_socket_path()).ok()?;

    let req = prompt_ipc::Request { cwd };
    stream.write_all(req.encode().as_bytes()).ok()?;

    let deadline = Instant::now() + Duration::from_millis(RESPONSE_TIMEOUT_MS);
    let mut reader = DeadlineReader::new(stream, deadline);
    read_response(&mut reader)
}

trait ReadTimeout {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()>;
}

impl ReadTimeout for UnixStream {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        UnixStream::set_read_timeout(self, timeout)
    }
}

struct DeadlineReader<R> {
    inner: R,
    deadline: Instant,
}

impl<R> DeadlineReader<R> {
    fn new(inner: R, deadline: Instant) -> Self {
        Self { inner, deadline }
    }
}

impl<R: Read + ReadTimeout> Read for DeadlineReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let Some(remaining) = self
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
        else {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "response deadline exceeded",
            ));
        };
        self.inner.set_read_timeout(Some(remaining))?;
        self.inner.read(buf)
    }
}

/// Maximum number of bytes accepted for a daemon response.
///
/// The socket is only trusted at the same-user boundary, so keep a defensive
/// cap to avoid unbounded allocation if another local process owns the socket.
/// This mirrors the daemon-side request cap and leaves ample room for prompt
/// status lines.
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Reads one newline-terminated response from the daemon.
///
/// A Unix socket `read()` is not guaranteed to return a full line in one call,
/// so long responses or short reads must be reassembled until newline or EOF.
/// If the configured read timeout fires, the read error becomes `None` and the
/// caller falls back to starting the daemon.
///
/// The common case, where a complete line fits in the first 512-byte read,
/// still decodes directly from the stack buffer without allocating.
fn read_response<R: Read>(reader: &mut R) -> Option<String> {
    let mut chunk = [0u8; 512];
    let n = reader.read(&mut chunk).ok()?;
    if n == 0 {
        return None;
    }
    // Hot path: the complete response line arrived in the first read.
    if chunk[..n].contains(&b'\n') {
        return prompt_ipc::Response::decode(&chunk[..n]).map(|r| r.output);
    }

    // Allocate only when the response line spans multiple reads.
    let mut buf = chunk[..n].to_vec();
    while buf.len() < MAX_RESPONSE_BYTES {
        let n = reader.read(&mut chunk).ok()?;
        if n == 0 {
            return prompt_ipc::Response::decode(&buf).map(|r| r.output);
        }

        let remaining = MAX_RESPONSE_BYTES - buf.len();
        let n = n.min(remaining);
        let slice = &chunk[..n];
        let reached_newline = slice.contains(&b'\n');
        buf.extend_from_slice(slice);
        if reached_newline {
            return prompt_ipc::Response::decode(&buf).map(|r| r.output);
        }
    }

    None
}

fn spawn_daemon() {
    // 自身のバイナリパス (merge-ready-prompt) と同じディレクトリにある merge-ready を探す。
    // current_exe / parent は実用上失敗しない前提で expect する。
    let exe = std::env::current_exe().expect("current_exe");
    let daemon_exe = exe.parent().expect("exe parent").join("merge-ready");

    // MERGE_READY_DAEMON_INNER=1 で outer wrapper をスキップして直接 inner として起動する。
    // この文字列は infrastructure::paths::DAEMON_INNER_ENV と同一でなければならない。
    // fire-and-forget: blocking しない
    let _ = std::process::Command::new(&daemon_exe)
        .args(["daemon", "start"])
        .env("MERGE_READY_DAEMON_INNER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::io;
    use std::rc::Rc;

    /// Test reader that returns at most one queued chunk per `read()` call.
    /// Remaining bytes are pushed back, and an empty queue returns EOF. This
    /// deterministically reproduces short reads from a Unix socket.
    struct ChunkedReader {
        chunks: VecDeque<Vec<u8>>,
    }

    impl ChunkedReader {
        fn new<I: IntoIterator<Item = Vec<u8>>>(chunks: I) -> Self {
            Self {
                chunks: chunks.into_iter().collect(),
            }
        }
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let Some(chunk) = self.chunks.pop_front() else {
                return Ok(0);
            };
            let n = chunk.len().min(buf.len());
            buf[..n].copy_from_slice(&chunk[..n]);
            if n < chunk.len() {
                self.chunks.push_front(chunk[n..].to_vec());
            }
            Ok(n)
        }
    }

    /// Test reader that simulates a read after `set_read_timeout` expires.
    struct TimeoutReader;

    impl Read for TimeoutReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "timed out"))
        }
    }

    struct RecordingReader {
        chunks: VecDeque<Vec<u8>>,
        timeouts: Rc<RefCell<Vec<Option<Duration>>>>,
    }

    impl RecordingReader {
        fn new<I: IntoIterator<Item = Vec<u8>>>(
            chunks: I,
            timeouts: Rc<RefCell<Vec<Option<Duration>>>>,
        ) -> Self {
            Self {
                chunks: chunks.into_iter().collect(),
                timeouts,
            }
        }
    }

    impl Read for RecordingReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let Some(chunk) = self.chunks.pop_front() else {
                return Ok(0);
            };
            let n = chunk.len().min(buf.len());
            buf[..n].copy_from_slice(&chunk[..n]);
            Ok(n)
        }
    }

    impl ReadTimeout for RecordingReader {
        fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
            self.timeouts.borrow_mut().push(timeout);
            Ok(())
        }
    }

    fn output_line(output: &str) -> Vec<u8> {
        let mut line = format!(r#"{{"tag":"output","output":"{output}"}}"#).into_bytes();
        line.push(b'\n');
        line
    }

    #[test]
    fn read_response_decodes_single_read_complete_line() {
        let mut reader = ChunkedReader::new([output_line("✓ Ready for merge")]);
        assert_eq!(
            read_response(&mut reader).as_deref(),
            Some("✓ Ready for merge")
        );
    }

    #[test]
    fn read_response_reassembles_line_split_across_reads() {
        let line = output_line("✓ Ready for merge #200 ✎ Ready for review #201");
        let mid = line.len() / 2;
        let mut reader = ChunkedReader::new([line[..mid].to_vec(), line[mid..].to_vec()]);
        assert_eq!(
            read_response(&mut reader).as_deref(),
            Some("✓ Ready for merge #200 ✎ Ready for review #201"),
        );
    }

    #[test]
    fn read_response_reads_response_longer_than_chunk() {
        let output = "x".repeat(600);
        let line = output_line(&output);
        assert!(line.len() > 512, "fixture must exceed the 512B chunk size");
        let chunks: Vec<Vec<u8>> = line.chunks(512).map(<[u8]>::to_vec).collect();
        let mut reader = ChunkedReader::new(chunks);
        assert_eq!(read_response(&mut reader).as_deref(), Some(output.as_str()));
    }

    #[test]
    fn read_response_returns_none_for_oversized_input_without_newline() {
        let oversized = vec![b'a'; MAX_RESPONSE_BYTES + 4096];
        let mut reader = ChunkedReader::new([oversized]);
        assert!(read_response(&mut reader).is_none());
    }

    #[test]
    fn read_response_rejects_oversized_complete_json_without_newline() {
        let mut line = format!(r#"{{"tag":"output","output":"{}"}}"#, "x".repeat(64)).into_bytes();
        line.extend(std::iter::repeat_n(b' ', MAX_RESPONSE_BYTES));
        let mut reader = ChunkedReader::new([line]);
        assert!(read_response(&mut reader).is_none());
    }

    #[test]
    fn read_response_returns_none_on_read_error() {
        let mut reader = TimeoutReader;
        assert!(read_response(&mut reader).is_none());
    }

    #[test]
    fn deadline_reader_refreshes_timeout_before_each_read() {
        let line = output_line("✓ Ready for merge");
        let mid = line.len() / 2;
        let timeouts = Rc::new(RefCell::new(Vec::new()));
        let inner = RecordingReader::new(
            [line[..mid].to_vec(), line[mid..].to_vec()],
            Rc::clone(&timeouts),
        );
        let mut reader =
            DeadlineReader::new(inner, std::time::Instant::now() + Duration::from_secs(10));

        assert_eq!(
            read_response(&mut reader).as_deref(),
            Some("✓ Ready for merge")
        );

        let recorded = timeouts.borrow();
        assert_eq!(recorded.len(), 2);
        assert!(recorded.iter().all(Option::is_some));
    }

    #[test]
    fn deadline_reader_returns_none_after_total_deadline() {
        let timeouts = Rc::new(RefCell::new(Vec::new()));
        let inner = RecordingReader::new([output_line("too late")], Rc::clone(&timeouts));
        let expired_deadline = std::time::Instant::now()
            .checked_sub(Duration::from_millis(1))
            .expect("1ms before now is representable");
        let mut reader = DeadlineReader::new(inner, expired_deadline);

        assert!(read_response(&mut reader).is_none());
        assert!(
            timeouts.borrow().is_empty(),
            "expired deadline must fail before updating per-read timeout"
        );
    }

    #[test]
    fn keeps_plain_text_unchanged() {
        assert_eq!(sanitize_output("✓ Ready for merge"), "✓ Ready for merge");
    }

    #[test]
    fn keeps_sgr_color_sequences() {
        // bold green + reset。E2E のスタイル機能が依存する正当な出力。
        let styled = "\u{1b}[1;32m✓\u{1b}[0m Ready";
        assert_eq!(sanitize_output(styled), styled);
    }

    #[test]
    fn keeps_ansi256_and_rgb_sgr() {
        let styled = "\u{1b}[38;5;196m✗\u{1b}[0m \u{1b}[38;2;255;0;0mfail\u{1b}[0m";
        assert_eq!(sanitize_output(styled), styled);
    }

    #[test]
    fn strips_c0_control_characters() {
        // ベル・NUL・キャリッジリターン・タブ・改行を除去する。
        assert_eq!(sanitize_output("a\u{07}b\u{00}c\rd\te\nf"), "abcdef");
    }

    #[test]
    fn strips_del_and_c1_controls() {
        assert_eq!(sanitize_output("a\u{7f}b\u{80}c\u{9f}d"), "abcd");
    }

    #[test]
    fn strips_cursor_movement_csi() {
        // CSI カーソル上移動 (`ESC [ 2 A`) は SGR ではないので除去する。
        assert_eq!(sanitize_output("a\u{1b}[2Ab"), "ab");
    }

    #[test]
    fn strips_screen_clear_csi() {
        // 画面消去 (`ESC [ 2 J`) を除去する。
        assert_eq!(sanitize_output("\u{1b}[2Jhello"), "hello");
    }

    #[test]
    fn strips_osc_escape() {
        // OSC（`ESC ]`）はウィンドウタイトル変更など。`ESC [` で始まらないため
        // ESC を捨て、続くテキストはそのまま残る（制御文字は別途除去される）。
        let injected = "\u{1b}]0;malicious\u{07}prompt";
        // ESC と BEL が除去され、残りは表示テキストとして残る。
        assert_eq!(sanitize_output(injected), "]0;maliciousprompt");
    }

    #[test]
    fn strips_lone_escape_at_end() {
        assert_eq!(sanitize_output("ok\u{1b}"), "ok");
    }

    #[test]
    fn strips_unterminated_csi() {
        // 終端されない CSI（EOF まで来る）は丸ごと除去する。
        assert_eq!(sanitize_output("ok\u{1b}[38;5;"), "ok");
    }

    #[test]
    fn keeps_loading_placeholder() {
        assert_eq!(sanitize_output("? loading"), "? loading");
    }
}
