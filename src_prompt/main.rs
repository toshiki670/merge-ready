// merge-ready-prompt: 軽量なシェルプロンプト用バイナリ。

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::time::Duration;

use merge_ready::{prompt_ipc, prompt_socket_path};

const READ_TIMEOUT_MS: u64 = 500;

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

    let stream = UnixStream::connect(prompt_socket_path()).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)))
        .ok()?;
    let mut stream = stream;

    let req = prompt_ipc::Request { cwd };
    stream.write_all(req.encode().as_bytes()).ok()?;

    // レスポンスはスタックバッファで受け取る（8KB BufReader ヒープ確保を回避）
    let mut buf = [0u8; 512];
    let n = stream.read(&mut buf).ok()?;

    prompt_ipc::Response::decode(&buf[..n]).map(|r| r.output)
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
