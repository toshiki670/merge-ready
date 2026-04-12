use std::fs;
use std::io::Write as _;

const CACHE_DIR_NAME: &str = "merge-ready";

/// リフレッシュロックを取得する。成功時は `true`、既に起動中なら `false` を返す。
///
/// `create_new(true)`（`O_CREAT | O_EXCL`）でアトミックにファイルを作成し、
/// 直後に自プロセスの PID を書き込む。これにより空ファイルが存在する瞬間をなくす。
///
/// ロックファイルが既存の場合は PID で生存確認（`kill -0`）を行い、
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
pub fn update_pid(repo_id: &str, pid: u32) {
    if let Some(path) = lock_path(repo_id) {
        let _ = fs::write(path, pid.to_string());
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

/// ロックファイルをアトミックに作成し、直後に自 PID を書き込む。
///
/// 作成に成功した場合 `true`、既に存在する場合 `false` を返す。
fn create_with_pid(path: &std::path::Path) -> bool {
    if let Ok(mut f) = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        let _ = f.write_all(std::process::id().to_string().as_bytes());
        true
    } else {
        false
    }
}

/// ロックファイルが示すプロセスが生存しているかを確認する。
///
/// - PID あり → `kill -0 <pid>` でプロセス生存確認
/// - PID なし（空ファイル）→ 異常状態として dead 扱い
fn is_alive(path: &std::path::Path) -> bool {
    let content = fs::read_to_string(path).unwrap_or_default();
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return false;
    }

    trimmed.parse::<u32>().is_ok_and(|pid| {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
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
