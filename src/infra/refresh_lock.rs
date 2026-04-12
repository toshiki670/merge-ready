use std::fs;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const CACHE_DIR_NAME: &str = "merge-ready";

/// PID 再利用安全弁としてのロック最大有効期間。
///
/// 子プロセスがクラッシュしてロックを解放できなかった場合、OS が同 PID を別プロセスに
/// 再利用すると `kill -0` が誤って true を返し続ける。`locked_at` との組み合わせで
/// 一定時間後に強制失効させることで影響を抑える。
///
/// gh コマンドのハング問題（#17）とは別問題。#17 で gh タイムアウトが確定したら再調整する。
const MAX_LOCK_AGE_SECS: u64 = 120;

#[derive(Serialize, Deserialize)]
struct LockFile {
    pid: u32,
    locked_at: u64,
}

/// リフレッシュロックを取得する。成功時は `true`、既に起動中なら `false` を返す。
///
/// `create_new(true)`（`O_CREAT | O_EXCL`）でアトミックにファイルを作成し、
/// 直後に自プロセスの PID と取得時刻を JSON で書き込む。
/// これにより空ファイルが存在する瞬間をなくす。
///
/// ロックファイルが既存の場合は PID と age で生存確認を行い、
/// プロセスが死んでいれば除去して再取得する。
pub fn try_acquire(repo_id: &str) -> bool {
    let Some(path) = lock_path(repo_id) else {
        return false;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if create_with_pid(&path) {
        return true;
    }

    // ロックファイルが既存: 生存確認して死んでいれば再取得
    if is_alive(&path) {
        return false;
    }
    let _ = fs::remove_file(&path);
    create_with_pid(&path)
}

/// spawn 後に子プロセスの PID をロックファイルへ上書きする。
///
/// `locked_at` をリセットして子プロセスの開始時刻を反映する。
pub fn update_pid(repo_id: &str, pid: u32) {
    if let Some(path) = lock_path(repo_id) {
        let lock = LockFile {
            pid,
            locked_at: now_secs(),
        };
        if let Ok(content) = serde_json::to_string(&lock) {
            let _ = fs::write(path, content);
        }
    }
}

/// リフレッシュロックを解放する。
pub fn release(repo_id: &str) {
    if let Some(path) = lock_path(repo_id) {
        let _ = fs::remove_file(path);
    }
}

/// ロックを取得できた場合のみバックグラウンドリフレッシュを起動する（多重起動抑止）。
pub fn maybe_spawn_refresh(repo_id: &str) {
    if !try_acquire(repo_id) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        release(repo_id);
        return;
    };
    match std::process::Command::new(exe)
        .args(["prompt", "--refresh"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => {
            // 子 PID をロックファイルへ書き込む（kill -0 による生存確認に使用）
            update_pid(repo_id, child.id());
        }
        Err(_) => {
            release(repo_id);
        }
    }
    // ロックは子プロセス（run_refresh の末尾）が解放する
}

/// ロックファイルをアトミックに作成し、ハンドルを保持したまま自 PID と取得時刻を JSON で書き込む。
///
/// `create_new(true)`（`O_CREAT | O_EXCL`）でアトミックにファイルを作成後、
/// ハンドルを閉じる前に `write_all` で JSON を書くことで「空ファイル」状態を排除する。
/// 書き込み失敗時はファイルを削除して `false` を返す。
///
/// 作成に成功した場合 `true`、既に存在する場合は `false` を返す。
fn create_with_pid(path: &std::path::Path) -> bool {
    let Ok(mut f) = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    else {
        return false;
    };
    let lock = LockFile {
        pid: std::process::id(),
        locked_at: now_secs(),
    };
    let Ok(content) = serde_json::to_string(&lock) else {
        drop(f);
        let _ = fs::remove_file(path);
        return false;
    };
    if f.write_all(content.as_bytes()).is_err() {
        drop(f);
        let _ = fs::remove_file(path);
        return false;
    }
    true
}

/// ロックファイルが示すプロセスが生存しているかを確認する。
///
/// - JSON パース失敗（空ファイル含む）→ dead 扱い
/// - `kill -0 <pid>` が失敗 → dead
/// - `now - locked_at >= MAX_LOCK_AGE_SECS` → dead（PID 再利用安全弁）
fn is_alive(path: &std::path::Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(lock) = serde_json::from_str::<LockFile>(&content) else {
        return false;
    };

    let age = now_secs().saturating_sub(lock.locked_at);
    if age >= MAX_LOCK_AGE_SECS {
        return false;
    }

    std::process::Command::new("kill")
        .args(["-0", &lock.pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn lock_path(repo_id: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::Path::new(&home)
            .join(".cache")
            .join(CACHE_DIR_NAME)
            .join(format!("{repo_id}.lock")),
    )
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tempfile::tempdir;

    use super::*;

    /// `create_with_pid` を N スレッドから同時に呼んでも、成功は正確に 1 つだけであること。
    ///
    /// `O_CREAT | O_EXCL` の OS アトミック性を検証する。
    #[test]
    fn create_with_pid_concurrent_exactly_one_succeeds() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.lock");
        let success_count = Arc::new(AtomicUsize::new(0));

        std::thread::scope(|s| {
            let handles: Vec<_> = (0..16)
                .map(|_| {
                    let count = Arc::clone(&success_count);
                    let p = path.clone();
                    s.spawn(move || {
                        if create_with_pid(&p) {
                            count.fetch_add(1, Ordering::SeqCst);
                        }
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
        });

        assert_eq!(
            success_count.load(Ordering::SeqCst),
            1,
            "exactly 1 thread should win create_with_pid"
        );
    }

    /// `create_with_pid` 成功直後、ファイルに有効な JSON が書き込まれており
    /// 現プロセスの PID が記録されていること（空ファイル状態が残らないことの確認）。
    #[test]
    fn create_with_pid_writes_valid_json_immediately() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.lock");

        assert!(create_with_pid(&path));

        let content = std::fs::read_to_string(&path).unwrap();
        let lock: LockFile =
            serde_json::from_str(&content).expect("lock file should contain valid JSON");
        assert_eq!(lock.pid, std::process::id(), "pid should match current process");
        assert!(lock.locked_at > 0, "locked_at should be non-zero");
    }

    /// `create_with_pid` が失敗した後（ファイル既存）にロックファイルが孤立しないこと。
    ///
    /// リリース後に再取得できることで、孤立ファイルがないことを確認する。
    #[test]
    fn create_with_pid_failure_leaves_no_orphan_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.lock");

        assert!(create_with_pid(&path), "first acquire should succeed");
        assert!(!create_with_pid(&path), "second acquire should fail");

        // ファイルはちょうど 1 つ存在（失敗がファイルを汚染していない）
        assert!(path.exists());

        // リリース後は再取得できる（孤立ファイルがないことの証明）
        std::fs::remove_file(&path).unwrap();
        assert!(
            create_with_pid(&path),
            "should be re-acquirable after release — no orphan file"
        );
    }

    /// `is_alive` は空ファイルを「死んでいる」と判定すること。
    ///
    /// JSON パース失敗 = 空ファイル含む → false。
    #[test]
    fn is_alive_returns_false_for_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.lock");
        std::fs::write(&path, b"").unwrap();
        assert!(!is_alive(&path));
    }
}
