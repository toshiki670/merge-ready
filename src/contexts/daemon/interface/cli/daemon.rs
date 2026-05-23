use std::path::{Path, PathBuf};
use std::process::{ExitCode, ExitStatus, Stdio};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::contexts::daemon::domain::daemon::{DaemonLifecyclePort, DaemonStatus};

const DAEMON_INNER_ENV: &str = "MERGE_READY_DAEMON_INNER";
const START_TIMEOUT_SECS: u64 = 2;

/// `supervise` が内側プロセスを監視した結果。`interpret` がこれを `Exit` へ変換する。
///
/// `ExitStatus` を直接持たず [`ExitInfo`] へ正規化することで、`interpret` を OS に
/// 依存しない純粋な分岐ロジックとして単体テストできる。
#[derive(Debug)]
enum StartResult {
    Ready,
    ExeNotFound,
    SpawnFailed(String),
    EarlyExit { exit: ExitInfo, stderr: String },
    Timeout(u64),
}

/// 内側プロセスの終了状態（`ExitStatus` を単体テストしやすい形へ正規化したもの）。
#[derive(Debug)]
enum ExitInfo {
    /// 正常終了（exit code 0）。`ready` を出力せずに成功した異常ケース。
    Success,
    /// 異常終了。`code` は終了コード（シグナル終了などで取得できなければ `None`）。
    Failure(Option<i32>),
    /// `wait()` 自体が OS レベルで失敗した。
    WaitError,
}

impl ExitInfo {
    fn from_wait(status: &std::io::Result<ExitStatus>) -> Self {
        match status {
            Ok(s) if s.success() => Self::Success,
            Ok(s) => Self::Failure(s.code()),
            Err(_) => Self::WaitError,
        }
    }
}

/// 終了コードの抽象表現。`ExitCode` は `PartialEq` を実装せず単体テストで検証できないため、
/// 比較可能な enum として表現し、境界で `ExitCode` へ変換する。
#[derive(Debug, PartialEq, Eq)]
enum Exit {
    Success,
    Failure,
    Code(u8),
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        match exit {
            Exit::Success => Self::SUCCESS,
            Exit::Failure => Self::FAILURE,
            Exit::Code(code) => Self::from(code),
        }
    }
}

/// `tokio::select!` の生の結果（stderr 読み取り・kill を行う前の段階）。
enum StartOutcome {
    Ready,
    EarlyExit(std::io::Result<ExitStatus>),
    Timeout,
}

// Why double-spawn instead of alternatives:
//
// - `daemonize` crate: RUSTSEC-2025-0069 (unmaintained) で cargo-deny に弾かれる
// - `libc`/`nix` の fork 直呼び: `unsafe_code = "forbid"` により使用不可
// - systemd/launchd: OS 依存。unit/plist ファイルの生成・登録が必要で複雑
//
// double-spawn は safe Rust のみで実現できる唯一の手段。
// 欠点は setsid() を呼べないため SIGHUP を受ける可能性があること。
// ただしプロンプト統合の用途では端末クローズ時にデーモンが終了しても
// 次回 prompt 呼び出し時に lazy_start() が再起動するため実害はない。
pub(crate) async fn start(port: &impl DaemonLifecyclePort) -> ExitCode {
    if std::env::var(DAEMON_INNER_ENV).is_ok() {
        return match port.start().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    launch(
        std::env::current_exe(),
        &["daemon", "start"],
        Duration::from_secs(START_TIMEOUT_SECS),
    )
    .await
    .into()
}

/// 外側プロセスの起動フロー。実行ファイルの解決結果とタイムアウトを引数に取ることで、
/// 実行ファイルが見つからない経路を単体テストで注入できる。
async fn launch(exe: std::io::Result<PathBuf>, args: &[&str], timeout: Duration) -> Exit {
    let result = match exe {
        Ok(exe) => supervise(&exe, args, timeout).await,
        Err(_) => StartResult::ExeNotFound,
    };
    interpret(result)
}

/// 内側プロセスを起動し、`ready` 出力・早期終了・タイムアウトのいずれかへ解決する。
async fn supervise(exe: &Path, args: &[&str], timeout: Duration) -> StartResult {
    let mut child = match Command::new(exe)
        .args(args)
        .env(DAEMON_INNER_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // inherit ではなく piped にする。inherit だと内側プロセスが外側の stderr fd
        // のコピーを保持したまま走り続けるため、assert_cmd が EOF 待ちでハングする。
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return StartResult::SpawnFailed(e.to_string()),
    };

    let Some(stdout) = child.stdout.take() else {
        // .stdout(Stdio::piped()) 指定済みなので take() は必ず Some を返す。
        unreachable!("stdout was configured with Stdio::piped()")
    };
    let Some(mut stderr) = child.stderr.take() else {
        // .stderr(Stdio::piped()) 指定済みなので take() は必ず Some を返す。
        unreachable!("stderr was configured with Stdio::piped()")
    };
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    let outcome = tokio::select! {
        res = reader.read_line(&mut line) => {
            if res.is_ok() && line == "ready\n" {
                StartOutcome::Ready
            } else {
                // EOF / 「ready\n」以外: 内側プロセスは既に終了したか終了直前。
                // 完全な exit status を取得するため明示的に wait する。
                StartOutcome::EarlyExit(child.wait().await)
            }
        }
        // 孫プロセスが stdout の書き込み端を保持したまま内側プロセスだけ終了した場合に拾う。
        // read_line 側が EOF を観測できないレースでのみ発火するため、決定的な再現が難しく
        // 単体テストでは覆えない。read_line→EOF 経路と機能的には等価。
        status = child.wait() => StartOutcome::EarlyExit(status),
        () = tokio::time::sleep(timeout) => StartOutcome::Timeout,
    };

    match outcome {
        StartOutcome::Ready => StartResult::Ready,
        StartOutcome::EarlyExit(status) => {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf).await;
            StartResult::EarlyExit {
                exit: ExitInfo::from_wait(&status),
                stderr: buf,
            }
        }
        StartOutcome::Timeout => {
            let _ = child.kill().await;
            StartResult::Timeout(timeout.as_secs())
        }
    }
}

/// 監視結果を終了コードへ変換し、必要なメッセージを出力する。純粋な分岐ロジック。
fn interpret(result: StartResult) -> Exit {
    match result {
        StartResult::Ready => {
            println!("daemon started");
            Exit::Success
        }
        StartResult::ExeNotFound => {
            eprintln!("merge-ready: failed to locate executable");
            Exit::Failure
        }
        StartResult::SpawnFailed(e) => {
            eprintln!("merge-ready: failed to spawn daemon: {e}");
            Exit::Failure
        }
        StartResult::EarlyExit { exit, stderr } => {
            if !stderr.is_empty() {
                eprint!("{stderr}");
            }
            match exit {
                // 内側が ready を出さず exit 0: 起動失敗扱いとして非 0 を返す安全弁。
                ExitInfo::Success => Exit::Code(1),
                ExitInfo::Failure(code) => Exit::Code(u8::try_from(code.unwrap_or(1)).unwrap_or(1)),
                ExitInfo::WaitError => Exit::Failure,
            }
        }
        StartResult::Timeout(secs) => {
            eprintln!("merge-ready: daemon did not start within {secs}s");
            Exit::Failure
        }
    }
}

pub(crate) async fn stop(port: &impl DaemonLifecyclePort) -> ExitCode {
    if port.stop().await {
        println!("daemon stopped");
    } else {
        eprintln!("daemon is not running");
    }
    ExitCode::SUCCESS
}

pub(crate) async fn status(port: &impl DaemonLifecyclePort) -> ExitCode {
    let line = match port.get_status().await {
        Some(s) => format_status(&s, port.get_pid().await),
        None => "not running".to_owned(),
    };
    println!("{line}");
    ExitCode::SUCCESS
}

/// 起動中デーモンのステータス行を組み立てる。PID 取得不能時は `-` を表示する。
fn format_status(s: &DaemonStatus, pid: Option<u32>) -> String {
    let pid = pid.map_or_else(|| "-".to_owned(), |p| p.to_string());
    format!(
        "running  pid={}  entries={}  uptime={}s  version={}",
        pid, s.entries, s.uptime_secs, s.version
    )
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt as _;

    use super::*;

    // ── interpret: 監視結果 → 終了コードの純粋な分岐 ───────────────────────────

    #[test]
    fn interpret_ready_is_success() {
        assert_eq!(interpret(StartResult::Ready), Exit::Success);
    }

    #[test]
    fn interpret_exe_not_found_is_failure() {
        assert_eq!(interpret(StartResult::ExeNotFound), Exit::Failure);
    }

    #[test]
    fn interpret_spawn_failed_is_failure() {
        assert_eq!(
            interpret(StartResult::SpawnFailed("boom".to_owned())),
            Exit::Failure
        );
    }

    #[test]
    fn interpret_early_exit_success_is_treated_as_failure_code_1() {
        // ready を出さず exit 0 でも、起動失敗として非 0 を返す。
        let result = StartResult::EarlyExit {
            exit: ExitInfo::Success,
            stderr: String::new(),
        };
        assert_eq!(interpret(result), Exit::Code(1));
    }

    #[test]
    fn interpret_early_exit_failure_propagates_code() {
        let result = StartResult::EarlyExit {
            exit: ExitInfo::Failure(Some(42)),
            stderr: "already running\n".to_owned(),
        };
        assert_eq!(interpret(result), Exit::Code(42));
    }

    #[test]
    fn interpret_early_exit_failure_without_code_defaults_to_1() {
        // シグナル終了などで code 不明の場合は 1 にフォールバックする。
        let result = StartResult::EarlyExit {
            exit: ExitInfo::Failure(None),
            stderr: String::new(),
        };
        assert_eq!(interpret(result), Exit::Code(1));
    }

    #[test]
    fn interpret_early_exit_wait_error_is_failure() {
        let result = StartResult::EarlyExit {
            exit: ExitInfo::WaitError,
            stderr: String::new(),
        };
        assert_eq!(interpret(result), Exit::Failure);
    }

    #[test]
    fn interpret_timeout_is_failure() {
        assert_eq!(interpret(StartResult::Timeout(2)), Exit::Failure);
    }

    // ── ExitInfo::from_wait: ExitStatus の正規化 ───────────────────────────────

    #[test]
    fn exit_info_from_wait_success() {
        let status = ExitStatus::from_raw(0);
        assert!(matches!(
            ExitInfo::from_wait(&Ok(status)),
            ExitInfo::Success
        ));
    }

    #[test]
    fn exit_info_from_wait_failure_captures_code() {
        // Unix の raw wait status は「終了コード << 8」。code 7 を表現する。
        let status = ExitStatus::from_raw(7 << 8);
        assert!(matches!(
            ExitInfo::from_wait(&Ok(status)),
            ExitInfo::Failure(Some(7))
        ));
    }

    #[test]
    fn exit_info_from_wait_error() {
        let err = Err(std::io::Error::other("wait failed"));
        assert!(matches!(ExitInfo::from_wait(&err), ExitInfo::WaitError));
    }

    // ── Exit → ExitCode 変換 ───────────────────────────────────────────────────

    #[test]
    fn exit_converts_to_exit_code() {
        // 変換がパニックしないことを確認する（ExitCode は比較不能のため値検証はできない）。
        let _ = ExitCode::from(Exit::Success);
        let _ = ExitCode::from(Exit::Failure);
        let _ = ExitCode::from(Exit::Code(3));
    }

    // ── launch: 実行ファイル未解決の経路（OS-level error injection）────────────

    #[tokio::test]
    async fn launch_returns_failure_when_exe_not_found() {
        // current_exe() が Err を返した状況を注入する。
        let exit = launch(
            Err(std::io::Error::other("no exe")),
            &["daemon", "start"],
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(exit, Exit::Failure);
    }

    // ── supervise: 実プロセスによる OS エラー・タイムアウト注入 ─────────────────

    #[tokio::test]
    async fn supervise_spawn_failure_returns_spawn_failed() {
        let result = supervise(
            Path::new("/nonexistent/merge-ready-binary"),
            &[],
            Duration::from_secs(5),
        )
        .await;
        assert!(matches!(result, StartResult::SpawnFailed(_)));
    }

    #[tokio::test]
    async fn supervise_times_out_and_kills_child() {
        // ready を出さず終了もしない子プロセス。タイムアウト経路を決定的に発火させる。
        let result = supervise(
            Path::new("/bin/sh"),
            &["-c", "sleep 30"],
            Duration::from_millis(100),
        )
        .await;
        assert!(matches!(result, StartResult::Timeout(_)));
    }

    #[tokio::test]
    async fn supervise_early_exit_captures_stderr_and_code() {
        // ready を出さず stderr に書いて非 0 で終了する子プロセス。
        let result = supervise(
            Path::new("/bin/sh"),
            &["-c", "printf 'oops\\n' >&2; exit 3"],
            Duration::from_secs(5),
        )
        .await;
        match result {
            StartResult::EarlyExit { exit, stderr } => {
                assert!(matches!(exit, ExitInfo::Failure(Some(3))));
                assert_eq!(stderr, "oops\n");
            }
            other => panic!("expected EarlyExit, got: {other:?}"),
        }
    }

    // ── format_status: PID 表示の分岐 ──────────────────────────────────────────

    fn sample_status() -> DaemonStatus {
        DaemonStatus {
            entries: 3,
            uptime_secs: 12,
            version: "1.2.3".to_owned(),
        }
    }

    #[test]
    fn format_status_shows_dash_when_pid_unavailable() {
        let line = format_status(&sample_status(), None);
        assert!(line.contains("pid=-"), "got: {line}");
        assert!(line.contains("entries=3"));
        assert!(line.contains("uptime=12s"));
        assert!(line.contains("version=1.2.3"));
    }

    #[test]
    fn format_status_shows_pid_when_available() {
        let line = format_status(&sample_status(), Some(4242));
        assert!(line.contains("pid=4242"), "got: {line}");
    }
}
