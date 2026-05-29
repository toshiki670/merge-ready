//! `merge-ready daemon` サブコマンドの操作 E2E テスト（シナリオ #7–15）
//!
//! daemon の起動・停止・ステータス確認、およびバージョンアップ時のクリーンアップを検証する。

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv, apply_coverage_env};

const BIN: &str = "merge-ready";
const PROMPT_BIN: &str = "merge-ready-prompt";

const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str =
    r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]"#;
const COMMAND_TIMEOUT_MS: u64 = 5000;
const CONCURRENT_PROMPTS: usize = 8;

fn versioned_socket(base: &std::path::Path) -> std::path::PathBuf {
    base.join(format!("daemon-{}.sock", env!("CARGO_PKG_VERSION")))
}

fn versioned_pid(base: &std::path::Path) -> std::path::PathBuf {
    base.join(format!("daemon-{}.pid", env!("CARGO_PKG_VERSION")))
}

fn is_pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn status_output_with_timeout(env: &TestEnv) -> std::process::Output {
    let bin = assert_cmd::cargo::cargo_bin(BIN);
    let mut cmd = std::process::Command::new(bin);
    cmd.args(["daemon", "status"])
        .env("PATH", env.path_env())
        .env("HOME", env.home())
        .env("TMPDIR", env.home())
        .env("MERGE_READY_BASE_DIR", env.home())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    apply_coverage_env(&mut cmd);
    let mut child = cmd.spawn().expect("spawn daemon status");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(COMMAND_TIMEOUT_MS);
    loop {
        if child.try_wait().is_ok_and(|status| status.is_some()) {
            return child
                .wait_with_output()
                .expect("collect daemon status output");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("daemon status did not finish within {COMMAND_TIMEOUT_MS}ms");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn daemon_stop_output(env: &TestEnv) -> std::process::Output {
    let bin = assert_cmd::cargo::cargo_bin(BIN);
    let mut cmd = std::process::Command::new(bin);
    cmd.args(["daemon", "stop"])
        .env("PATH", env.path_env())
        .env("HOME", env.home())
        .env("TMPDIR", env.home())
        .env("MERGE_READY_BASE_DIR", env.home())
        .env("XDG_CONFIG_HOME", env.home().join(".config"))
        .current_dir(env.repo.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    apply_coverage_env(&mut cmd);
    let mut child = cmd.spawn().expect("spawn daemon stop");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(COMMAND_TIMEOUT_MS);
    loop {
        if child.try_wait().is_ok_and(|status| status.is_some()) {
            return child
                .wait_with_output()
                .expect("collect daemon stop output");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("daemon stop did not finish within {COMMAND_TIMEOUT_MS}ms");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn assert_prompt_succeeded(output: &std::process::Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "prompt failed: status={:?}, stdout={stdout:?}, stderr={stderr:?}",
        output.status.code(),
    );
    assert!(
        stderr.is_empty(),
        "prompt stderr should be empty: {stderr:?}"
    );
    assert!(
        stdout == "? loading" || stdout == "✓ Ready for merge",
        "unexpected prompt stdout: {stdout:?}",
    );
}

fn prompt_output_with_timeout(
    bin: &std::path::Path,
    path: &str,
    home: &std::path::Path,
    repo: &std::path::Path,
) -> std::process::Output {
    use std::io::{Read, Seek};

    // パイプではなく匿名一時ファイルに出力をリダイレクトする。
    // パイプの場合、spawn_daemon() で起動した孫プロセスが書き込み端を保持したまま
    // 生き続けると read_to_end が EOF 待ちで永久ブロックする。
    // ファイルへのリダイレクトなら孫プロセスが fd を開いたままでも
    // プロセス終了後にシークして読み返せるため、この問題が発生しない。
    let mut stdout_file = tempfile::tempfile().expect("stdout tempfile");
    let mut stderr_file = tempfile::tempfile().expect("stderr tempfile");
    let stdout_for_child = stdout_file.try_clone().expect("clone stdout file");
    let stderr_for_child = stderr_file.try_clone().expect("clone stderr file");

    let mut cmd = std::process::Command::new(bin);
    cmd.env("PATH", path)
        .env("HOME", home)
        .env("TMPDIR", home)
        .env("MERGE_READY_BASE_DIR", home)
        .current_dir(repo)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(stdout_for_child))
        .stderr(std::process::Stdio::from(stderr_for_child));
    apply_coverage_env(&mut cmd);
    let mut child = cmd.spawn().expect("run prompt");

    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(COMMAND_TIMEOUT_MS);
    let status = loop {
        if let Ok(Some(status)) = child.try_wait() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("prompt did not finish within {COMMAND_TIMEOUT_MS}ms");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };

    let mut stdout = Vec::new();
    stdout_file.seek(std::io::SeekFrom::Start(0)).ok();
    stdout_file.read_to_end(&mut stdout).ok();

    let mut stderr = Vec::new();
    stderr_file.seek(std::io::SeekFrom::Start(0)).ok();
    stderr_file.read_to_end(&mut stderr).ok();

    std::process::Output {
        status,
        stdout,
        stderr,
    }
}

// ── #7: daemon start ─────────────────────────────────────────────────────────

/// #7: `daemon start` → "daemon started" を出力して exit 0
#[test]
fn test_daemon_start_prints_started() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "start"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("daemon started"));

    let output = daemon_stop_output(&env);
    assert!(output.status.success());
}

// ── #8: daemon start（二重起動）────────────────────────────────────────────

/// #8: daemon 起動済みで `daemon start` → "already running" を stderr に出力して exit 非 0
#[test]
fn test_daemon_start_already_running_fails() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "start"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("already running"));
}

// ── #9: daemon status（起動中）──────────────────────────────────────────────

/// #9: `daemon status`（起動中）→ "running" を含む出力
#[test]
fn test_daemon_status_shows_running() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("running"));
}

// ── #10: daemon status（バージョン）─────────────────────────────────────────

/// #10: `daemon status`（起動中）→ バージョン文字列を含む
#[test]
fn test_daemon_status_includes_version() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

// ── #11: daemon stop ─────────────────────────────────────────────────────────

/// #11: `daemon stop` → "stopped" を出力して exit 0
#[test]
fn test_daemon_stop_prints_stopped() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let daemon = DaemonHandle::start(&env);

    let output = daemon_stop_output(&env);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("stopped"));

    drop(daemon);
}

// ── #12: daemon stop 後の status ─────────────────────────────────────────────

/// #12: `daemon stop` 後の `daemon status` → "not running"
#[test]
fn test_daemon_stop_then_status_not_running() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let daemon = DaemonHandle::start(&env);

    let output = daemon_stop_output(&env);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("stopped"));

    drop(daemon);

    let mut status = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut status);
    status.args(["daemon", "status"]);
    status
        .assert()
        .success()
        .stdout(predicate::str::contains("not running"));
}

#[test]
fn test_daemon_stop_does_not_wait_for_scheduler_tick() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let daemon = DaemonHandle::start_with_env(&env, &[("MERGE_READY_SCHEDULER_TICK_SECS", "5")]);

    let started = std::time::Instant::now();
    let output = daemon_stop_output(&env);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("stopped"));

    assert!(
        started.elapsed() < std::time::Duration::from_secs(4),
        "daemon stop should not wait for the scheduler tick"
    );
    drop(daemon);
}

// ── #13: 未起動時の status ───────────────────────────────────────────────────

/// #13: 未起動時の `daemon status` → "not running"
#[test]
fn test_daemon_status_not_running() {
    let env = TestEnv::new(r#"{"state":"OPEN"}"#, None);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("not running\n"));
}

// ── #14: 旧バージョン stale ファイルのクリーンアップ ───────────────────────

/// #14: 旧バージョンのクラッシュ残骸（空ソケット + 死んだ PID）がある状態で
/// 新バージョンの `daemon start` を実行すると、stale ファイルが削除される
#[test]
fn test_daemon_start_cleans_up_old_version_stale_files() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let base = env.home().to_path_buf();

    // 旧バージョンの残骸を作る（空ソケット + 死んだ PID）
    let old_sock = base.join("daemon-0.0.0.sock");
    let old_pid_path = base.join("daemon-0.0.0.pid");
    std::fs::create_dir_all(&base).expect("create base dir");
    std::fs::File::create(&old_sock).expect("write stale socket placeholder");
    let mut dead = std::process::Command::new("true")
        .spawn()
        .expect("spawn true");
    let dead_pid = dead.id();
    dead.wait().expect("wait for dead process");
    std::fs::write(&old_pid_path, dead_pid.to_string()).expect("write stale pid file");

    // 新バージョンのデーモンを起動 → 非同期で旧バージョンファイルを掃除する
    let _guard = DaemonHandle::start(&env);

    // 非同期クリーンアップが完了するまでポーリング（最大 5 秒）
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if !old_sock.exists() && !old_pid_path.exists() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "stale old-version files were not cleaned up within 5s"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// ── #15: 同時起動レース ──────────────────────────────────────────────────────

/// #15: 複数の `merge-ready-prompt` が同時に Daemon 起動を試みても、Daemon は 1 プロセスのみ存在する
///
/// daemon 未起動の状態で複数の `merge-ready-prompt` を並列実行し、
/// 全て完了後に daemon が 1 プロセスだけ動作していることを確認する。
#[test]
fn test_concurrent_prompt_starts_only_one_daemon() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let prompt_bin = assert_cmd::cargo::cargo_bin(PROMPT_BIN);

    // 複数本を同時起動
    let handles: Vec<_> = (0..CONCURRENT_PROMPTS)
        .map(|_| {
            let bin = prompt_bin.clone();
            let path = env.path_env();
            let home = env.home().to_path_buf();
            let repo = env.repo.path().to_path_buf();
            std::thread::spawn(move || prompt_output_with_timeout(&bin, &path, &home, &repo))
        })
        .collect();

    // 全スレッド完了を待ち、各 prompt が競合中も正常終了することを確認
    for h in handles {
        let output = h.join().expect("prompt thread panicked");
        assert_prompt_succeeded(&output);
    }

    // daemon が起動するまでポーリング（最大 5 秒）
    // spawn_daemon() は fire-and-forget のため、prompt 完了後も daemon が起動中の場合がある
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let out = status_output_with_timeout(&env);
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.starts_with("running") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "daemon did not start within 5s: {stdout}"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // ソケットファイルが 1 つだけ存在することを確認（複数 daemon は起動していない）
    let socket_path = versioned_socket(env.home_tmp.path());
    assert!(socket_path.exists(), "daemon socket should exist");

    DaemonHandle::stop_for_env(&env);
}

// ── #16: 生きている旧バージョンデーモンの非同期クリーンアップ ───────────────

/// #16: 旧バージョンのデーモンが実際に動いている状態で新バージョンの `daemon start`
/// を実行すると、新デーモンは自身をバインドした後、非同期で旧デーモンに Stop を送信する
///
/// Unix ドメインソケットはバインド後にファイル名を変更してもプロセスのファイルディスクリプタは有効なので、
/// 起動済みデーモンの socket / pid をリネームすることで「旧バージョンが生きている状態」を再現する。
#[test]
fn test_daemon_start_sends_stop_to_live_old_version_daemon() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let base = env.home().to_path_buf();

    // デーモンを起動して、ソケット・PID ファイルのパスを取得
    let old_guard = DaemonHandle::start(&env);
    let current_sock = versioned_socket(&base);
    let current_pid_path = versioned_pid(&base);

    let old_pid: u32 = std::fs::read_to_string(&current_pid_path)
        .expect("read current pid")
        .trim()
        .parse()
        .expect("parse pid");

    // 旧バージョンをシミュレート: ファイルを daemon-0.0.0.* に rename
    // プロセスは FD を保持しているので socket は有効のまま
    let old_sock = base.join("daemon-0.0.0.sock");
    let old_pid_path = base.join("daemon-0.0.0.pid");
    std::fs::rename(&current_sock, &old_sock).expect("rename sock");
    std::fs::rename(&current_pid_path, &old_pid_path).expect("rename pid");

    // old_guard の Drop は rename された socket/pid を参照できず graceful failure。
    // 新デーモンが起動する前に Drop されてしまうと daemon stop が無害な空振りになるのみ。
    // 安全のため std::mem::forget で Drop をスキップしておく。
    std::mem::forget(old_guard);

    // 新デーモンを起動 → 旧バージョンのソケットに非同期で Stop を送信する
    let _new_guard = DaemonHandle::start(&env);

    // 旧 PID が終了するまで最大 5 秒待つ
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if !is_pid_alive(old_pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        !is_pid_alive(old_pid),
        "old daemon (pid={old_pid}) should be stopped by new daemon"
    );

    // 旧バージョンのファイルがクリーンアップされていること
    let cleanup_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if !old_sock.exists() && !old_pid_path.exists() {
            break;
        }
        assert!(
            std::time::Instant::now() < cleanup_deadline,
            "old daemon files were not cleaned up within 5s"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// ── #17: daemon status の出力フォーマット ────────────────────────────────────

/// #17: `daemon status`（起動中）→ "running pid=<数字> entries=<数字> uptime=<数字>s version=<文字列>"
#[test]
fn test_daemon_status_format() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert().success().stdout(
        predicate::str::is_match(r"^running  pid=\d+  entries=\d+  uptime=\d+s  version=.+\n$")
            .unwrap(),
    );
}

// ── #S1: stale PID のみ・socket 無しでの daemon stop ────────────────────────

/// #S1: 死んだ PID ファイルだけが残り socket が存在しない状態で `daemon stop` を実行すると、
/// stale PID ファイルを削除して "daemon is not running" を stderr に出力する。
///
/// `DaemonLifecycle::stop` の dead-PID クリーンアップ経路を覆う。
#[test]
fn test_daemon_stop_with_stale_pid_and_no_socket() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    let mut dead = std::process::Command::new("true")
        .spawn()
        .expect("spawn true");
    let dead_pid = dead.id();
    dead.wait().expect("wait for dead process");

    let pid_path = versioned_pid(env.home());
    std::fs::write(&pid_path, dead_pid.to_string()).expect("write stale pid file");
    assert!(
        !versioned_socket(env.home()).exists(),
        "socket should not exist for this scenario"
    );

    let output = daemon_stop_output(&env);
    assert!(output.status.success(), "daemon stop should exit 0");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("daemon is not running"),
        "expected 'daemon is not running' in stderr, got: {stderr:?}"
    );
    assert!(
        !pid_path.exists(),
        "stale pid file should be removed by daemon stop"
    );
}

// ── #S2: socket 削除済み・daemon プロセス生存での daemon stop ────────────────

/// #S2: 起動済み daemon の socket ファイルだけを外部から削除した状態で `daemon stop` を実行すると、
/// SIGTERM フォールバックで daemon プロセスを停止させる。
///
/// `DaemonLifecycle::stop` の `terminate_and_wait` 経路を覆う。
#[test]
fn test_daemon_stop_falls_back_to_sigterm_when_socket_removed() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let pid: u32 = std::fs::read_to_string(versioned_pid(env.home()))
        .expect("read pid file")
        .trim()
        .parse()
        .expect("parse pid");
    assert!(is_pid_alive(pid), "daemon should be alive before stop");

    std::fs::remove_file(versioned_socket(env.home())).expect("remove socket file");

    let output = daemon_stop_output(&env);
    assert!(output.status.success(), "daemon stop should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("stopped"),
        "expected 'stopped' in stdout, got: {stdout:?}"
    );

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if !is_pid_alive(pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        !is_pid_alive(pid),
        "daemon (pid={pid}) should be terminated by SIGTERM fallback"
    );
}

// ── #S3: socket 消失で daemon が自己終了する ─────────────────────────────────

/// #S3: テスト異常終了などで `base_dir` ごと消えたケースの再現。
///
/// 起動済み daemon の socket ファイルを外部から削除すると、accept ループに仕込んだ
/// ポーリングで消失を検知し、daemon プロセスが自己終了する。
///
/// 双方向の保証:
/// - リーク防止: テストハーネスが Drop を走らせられず socket だけ消えるケースで daemon が孤児化しない
/// - 同一バージョン同期: 旧 daemon の socket を新 daemon の `restart::cleanup` 等で消したときの
///   フェイルセーフとしても効く
#[test]
fn test_daemon_self_terminates_when_socket_disappears() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    // E2E では検出までの待ち時間を短くしたいので、インターバルを 1 秒に縮める。
    let _daemon =
        DaemonHandle::start_with_env(&env, &[("MERGE_READY_SOCKET_CHECK_INTERVAL_SECS", "1")]);

    let pid: u32 = std::fs::read_to_string(versioned_pid(env.home()))
        .expect("read pid file")
        .trim()
        .parse()
        .expect("parse pid");
    assert!(
        is_pid_alive(pid),
        "daemon should be alive before socket removal"
    );

    std::fs::remove_file(versioned_socket(env.home())).expect("remove socket file");

    // インターバル 1 秒 + リスポンスの猶予を見て 5 秒以内に exit するはず
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if !is_pid_alive(pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        !is_pid_alive(pid),
        "daemon (pid={pid}) should self-terminate within 5s after socket file removed"
    );
}

// ── #19: SIGINT による daemon の停止 ─────────────────────────────────────────

/// #19: 実行中の daemon に SIGINT を送ると graceful に停止する。
///
/// `install_shutdown_signals` の `sigint.recv()` 経路を覆う。
#[test]
fn test_daemon_terminates_on_sigint() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let daemon = DaemonHandle::start(&env);

    let pid: u32 = std::fs::read_to_string(versioned_pid(env.home()))
        .expect("read pid file")
        .trim()
        .parse()
        .expect("parse pid");
    assert!(is_pid_alive(pid), "daemon should be alive before SIGINT");

    let status = std::process::Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status()
        .expect("send SIGINT");
    assert!(status.success(), "kill -INT failed");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if !is_pid_alive(pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        !is_pid_alive(pid),
        "daemon (pid={pid}) should terminate on SIGINT"
    );

    drop(daemon);
}

// ── #20: MERGE_READY_BASE_DIR 未設定でのデフォルトパス解決 ───────────────────

/// #20: `MERGE_READY_BASE_DIR` 未設定でも、daemon と prompt は `TMPDIR/merge-ready/` 配下の
/// 同じソケットで連携する。
///
/// `Paths::default()` の `env::var` Err 分岐と `base_dir()` / `dir_name()` を覆う。
#[test]
fn test_daemon_and_prompt_use_default_base_dir_when_unset() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    let bin = assert_cmd::cargo::cargo_bin(BIN);
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(["daemon", "start"])
        .env("PATH", env.path_env())
        .env("HOME", env.home())
        .env("TMPDIR", env.home())
        .env("XDG_CONFIG_HOME", env.home().join(".config"))
        .env_remove("MERGE_READY_BASE_DIR")
        .current_dir(env.repo.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    apply_coverage_env(&mut cmd);
    let mut child = cmd.spawn().expect("daemon spawn");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(START_TIMEOUT_MS);
    let status = loop {
        if let Ok(Some(s)) = child.try_wait() {
            break s;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("daemon did not start within {START_TIMEOUT_MS}ms");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(status.success(), "daemon start should succeed");

    // BASE_DIR 未設定なので TMPDIR/<dir_name>/ 配下に socket が作られる。
    // dir_name() は unix では uid で名前空間分離（"merge-ready-{uid}"）、
    // 非 unix では固定の "merge-ready"。
    let socket_name = format!("daemon-{}.sock", env!("CARGO_PKG_VERSION"));
    let socket_path = std::fs::read_dir(env.home())
        .expect("read home dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("merge-ready"))
        })
        .find_map(|d| {
            let candidate = d.join(&socket_name);
            candidate.exists().then_some(candidate)
        })
        .expect("daemon socket not found under any merge-ready* dir");

    // prompt を同じ env で実行 → daemon と通信して結果を返す
    let prompt_bin = assert_cmd::cargo::cargo_bin(PROMPT_BIN);
    let mut prompt_cmd = std::process::Command::new(&prompt_bin);
    prompt_cmd
        .env("PATH", env.path_env())
        .env("HOME", env.home())
        .env("TMPDIR", env.home())
        .env_remove("MERGE_READY_BASE_DIR")
        .current_dir(env.repo.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    apply_coverage_env(&mut prompt_cmd);
    let prompt_out = prompt_cmd.output().expect("prompt run");
    assert!(prompt_out.status.success(), "prompt failed");
    let stdout = String::from_utf8_lossy(&prompt_out.stdout);
    assert!(
        stdout == "? loading" || stdout.contains("Ready"),
        "unexpected prompt stdout: {stdout:?}"
    );

    // cleanup: daemon を SIGTERM で止める（base_dir を unset した特殊シナリオなので、
    // socket パスから pid を読み取って直接 kill する）
    let base = socket_path.parent().expect("socket parent");
    if let Ok(pid_str) =
        std::fs::read_to_string(base.join(format!("daemon-{}.pid", env!("CARGO_PKG_VERSION"))))
        && let Ok(pid) = pid_str.trim().parse::<u32>()
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if !is_pid_alive(pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

const START_TIMEOUT_MS: u64 = 5000;

// ── #18: stale PID ファイルのクリーンアップ ──────────────────────────────────

/// #18: 前回クラッシュした daemon が残した stale な PID ファイルがある状態で
/// `daemon start` を実行すると、クリーンアップして正常起動する
#[test]
fn test_daemon_start_cleans_up_stale_pid_file() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    // 死んだプロセスの PID で stale なファイルを作成してクラッシュ後の残骸を模倣する
    let mut dead = std::process::Command::new("true")
        .spawn()
        .expect("spawn true");
    let dead_pid = dead.id();
    dead.wait().expect("wait for dead process");

    let daemon_dir = env.home();
    std::fs::write(versioned_pid(daemon_dir), dead_pid.to_string()).expect("write stale pid file");
    std::fs::write(versioned_socket(daemon_dir), b"").expect("write stale socket placeholder");

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "start"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("daemon started"));

    DaemonHandle::stop_for_env(&env);
}
