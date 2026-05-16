use std::time::Duration;

use super::daemon_client::DaemonClient;
use super::paths::Paths;
use super::pid;

/// 旧バージョンのデーモン停止待ちタイムアウト。
const OLD_DAEMON_STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// 現バージョンの socket/pid ファイルを削除する。
///
/// `daemon_server::run` が異常終了したときの後始末で使う。
/// 正常終了（Stop 経由）でも `connection::handle` がこれを呼ぶ。
pub(super) fn cleanup(paths: &Paths) {
    let _ = std::fs::remove_file(paths.socket_path());
    pid::remove(&paths.pid_path());
}

/// 旧バージョンのデーモンファイルを非同期でクリーンアップする。
///
/// - PID が生きていれば socket 経由で Stop を送り、終了を待つ
/// - PID が死んでいる stale ファイルは即削除する
///
/// 新デーモンが自分のソケットを bind した後にバックグラウンドスレッドで実行することで、
/// `merge-ready-prompt` のレスポンスを旧デーモン停止の完了まで待たせない。
pub(super) fn cleanup_old_versions(paths: &Paths) {
    for old_pid_path in paths.old_daemon_pid_files() {
        cleanup_one(&old_pid_path);
    }
}

fn cleanup_one(old_pid_path: &std::path::Path) {
    let Some(stem) = old_pid_path.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    let old_sock = old_pid_path.with_file_name(format!("{stem}.sock"));

    match pid::read(old_pid_path) {
        Some(p) if pid::is_alive(p) => {
            let client = DaemonClient::new(old_sock.clone());
            let _ = client.stop();
            // 旧デーモンが Stop を完了するまで待ち、ファイルを掃除する
            let _ = pid::wait_until_gone(p, old_pid_path, OLD_DAEMON_STOP_TIMEOUT);
            let _ = std::fs::remove_file(&old_sock);
            // pid::wait_until_gone は成功時に pid ファイルを削除するが、失敗時にも残骸を残さない
            let _ = std::fs::remove_file(old_pid_path);
        }
        _ => {
            // stale: ファイルだけ削除
            let _ = std::fs::remove_file(&old_sock);
            let _ = std::fs::remove_file(old_pid_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn cleanup_old_versions_removes_stale_files() {
        let dir = tempfile::tempdir().unwrap();
        let version = env!("CARGO_PKG_VERSION");

        // 現バージョン（残す）
        fs::write(
            dir.path().join(format!("daemon-{version}.sock")),
            b"current",
        )
        .unwrap();
        fs::write(dir.path().join(format!("daemon-{version}.pid")), b"1").unwrap();

        // 旧バージョン stale（削除対象、死んだ PID）
        fs::write(dir.path().join("daemon-0.0.0.sock"), b"old").unwrap();
        fs::write(dir.path().join("daemon-0.0.0.pid"), b"9999999").unwrap();

        let paths = Paths::new(dir.path().to_path_buf());
        cleanup_old_versions(&paths);

        // 旧バージョンは削除
        assert!(!dir.path().join("daemon-0.0.0.sock").exists());
        assert!(!dir.path().join("daemon-0.0.0.pid").exists());
        // 現バージョンは残る
        assert!(dir.path().join(format!("daemon-{version}.sock")).exists());
        assert!(dir.path().join(format!("daemon-{version}.pid")).exists());
    }
}
