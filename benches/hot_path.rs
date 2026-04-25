//! ホットパスのベンチマーク
//!
//! ## 計測方針
//!
//! **対象**: `merge-ready-prompt` の「ウォーム起動時間」
//!   - daemon が起動済みで、対象リポジトリのキャッシュが温まった状態での
//!     `merge-ready-prompt` プロセス生成〜stdout 出力完了までの時間。
//!
//! **計測範囲に含むもの**:
//!   - OS プロセス生成（fork/exec）
//!   - ELF ロードと Rust ランタイム初期化
//!   - Unix socket 接続・クエリ送信・レスポンス受信
//!   - stdout への出力
//!
//! **計測範囲に含まないもの**:
//!   - daemon の起動（セットアップ段階で完了させる）
//!   - キャッシュ投入（セットアップ段階で完了させる）
//!   - `merge-ready` 本体（GH API 呼び出し等）の処理時間
//!
//! **合否基準**: [`THRESHOLD_SAMPLES`] サンプルの中央値 ≤ [`WARM_STARTUP_THRESHOLD_MS`] ms（#139 受け入れ条件）
//!   中央値を使うことで、OS スケジューラや GC による外れ値に左右されない判定を行う。
//!   CI では `cargo bench --bench hot_path -- --test` を実行して
//!   コンパイル・動作確認のみ行い、基準値チェックはローカルで行う。
//!
//! ## 使用方法
//!
//! ```sh
//! # ベンチマーク計測 + 閾値チェック（ローカル）
//! cargo bench --bench hot_path
//!
//! # 動作確認のみ（CI 用）
//! cargo bench --bench hot_path -- --test
//! ```

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;

/// Issue #139 の受け入れ条件: ウォーム起動時間の中央値上限（ミリ秒）
///
/// Linux はプロセス生成コストが低いため厳しい基準を設定する。
/// macOS は SIP/Gatekeeper によるオーバーヘッドがあるため緩い基準とする。
#[cfg(target_os = "linux")]
const WARM_STARTUP_THRESHOLD_MS: u64 = 10;
#[cfg(not(target_os = "linux"))]
const WARM_STARTUP_THRESHOLD_MS: u64 = 15;

/// 閾値判定に使うサンプル数。中央値を安定させるため奇数にする
const THRESHOLD_SAMPLES: usize = 21;

struct BenchEnv {
    bin_dir: TempDir,
    tmp_dir: TempDir,
    repo_dir: TempDir,
    daemon_process: Option<std::process::Child>,
}

impl Drop for BenchEnv {
    fn drop(&mut self) {
        let bin = binary_path("merge-ready");
        let _ = Command::new(&bin)
            .args(["daemon", "stop"])
            .env("TMPDIR", self.tmp_dir.path())
            .output();
        if let Some(mut child) = self.daemon_process.take() {
            let _ = child.kill();
        }
    }
}

const FAKE_BRANCH: &str = "main";
const OPEN_MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

fn write_executable(path: &PathBuf, content: &str) {
    fs::write(path, content).expect("write");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn path_env(env: &BenchEnv) -> String {
    format!("{}:/bin:/usr/bin", env.bin_dir.path().display())
}

fn binary_path(name: &str) -> PathBuf {
    let mut path = std::env::current_exe().expect("current_exe");
    path.pop(); // hot_path-xxx
    path.pop(); // deps
    path.push(name);
    if !path.exists() {
        path.pop();
        path.pop();
        path.push("release");
        path.push(name);
    }
    path
}

fn dir_name() -> String {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata("/proc/self") {
            return format!("merge-ready-{}", meta.uid());
        }
    }
    "merge-ready".to_owned()
}

/// daemon 起動 + キャッシュウォームまでを完了させるセットアップ。
/// `b.iter()` の外で呼ぶことで、ベンチマーク計測に daemon 起動コストを混入させない。
fn setup_bench_env() -> BenchEnv {
    let bin_dir = TempDir::new().expect("bin_dir");
    let tmp_dir = TempDir::new().expect("tmp_dir");
    let repo_dir = TempDir::new().expect("repo_dir");

    let gh_script = "#!/bin/sh\ncase \"$*\" in\n  *'pr view'*) printf '%s' '".to_owned()
        + OPEN_MERGE_READY_JSON
        + "' ;;\n  *'pr checks'*) printf '%s' '"
        + CI_PASS_JSON
        + "' ;;\n  *'api'*'compare'*) printf '{\"behind_by\":0}' ;;\n  *) exit 0 ;;\nesac\n";
    write_executable(&bin_dir.path().join("gh"), &gh_script);

    let git_dir = repo_dir.path().join(".git");
    fs::create_dir_all(&git_dir).expect("create .git");
    fs::write(
        git_dir.join("HEAD"),
        format!("ref: refs/heads/{FAKE_BRANCH}\n"),
    )
    .expect("write HEAD");

    let mut env = BenchEnv {
        bin_dir,
        tmp_dir,
        repo_dir,
        daemon_process: None,
    };

    // daemon を起動する（ここはセットアップ; 計測対象外）
    let bin = binary_path("merge-ready");
    let child = Command::new(&bin)
        .args(["daemon", "start"])
        .env("PATH", path_env(&env))
        .env("TMPDIR", env.tmp_dir.path())
        .env("HOME", env.tmp_dir.path())
        .current_dir(env.repo_dir.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("daemon spawn");
    env.daemon_process = Some(child);

    // socket 出現を待つ（計測対象外）
    let socket = env.tmp_dir.path().join(dir_name()).join("daemon.sock");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(3000);
    while std::time::Instant::now() < deadline {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(socket.exists(), "daemon did not start within 3s");

    // キャッシュが温まるまで待つ（計測対象外）
    let prompt_bin = binary_path("merge-ready-prompt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(5000);
    loop {
        let out = Command::new(&prompt_bin)
            .env("PATH", path_env(&env))
            .env("TMPDIR", env.tmp_dir.path())
            .current_dir(env.repo_dir.path())
            .output()
            .expect("merge-ready-prompt failed");
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout != "? loading" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "cache did not warm within 5s"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    env
}

/// [`THRESHOLD_SAMPLES`] サンプルの中央値が [`WARM_STARTUP_THRESHOLD_MS`] ms 以下であることを検証する。
///
/// 中央値を使うことで、OS スケジューラや GC による一時的な外れ値が判定を歪めない。
fn assert_warm_startup_threshold(env: &BenchEnv) {
    let prompt_bin = binary_path("merge-ready-prompt");
    let path = path_env(env);
    let tmpdir = env.tmp_dir.path().to_owned();
    let repo_dir = env.repo_dir.path().to_owned();

    let mut times: Vec<u128> = (0..THRESHOLD_SAMPLES)
        .map(|_| {
            let start = std::time::Instant::now();
            Command::new(&prompt_bin)
                .env("PATH", &path)
                .env("TMPDIR", &tmpdir)
                .current_dir(&repo_dir)
                .output()
                .expect("merge-ready-prompt failed");
            start.elapsed().as_millis()
        })
        .collect();

    times.sort_unstable();
    let median = times[THRESHOLD_SAMPLES / 2];

    assert!(
        median <= u128::from(WARM_STARTUP_THRESHOLD_MS),
        "中央値 {median}ms が閾値 {WARM_STARTUP_THRESHOLD_MS}ms を超えています\nsamples: {times:?}"
    );
}

/// ウォーム起動時間のベンチマーク（合否基準: 中央値 ≤ [`WARM_STARTUP_THRESHOLD_MS`] ms）
///
/// daemon 起動済み・キャッシュウォーム状態での `merge-ready-prompt` の
/// プロセス生成〜stdout 出力完了までを計測する。
/// セットアップ（daemon 起動・キャッシュ投入）は `b.iter()` の外で完了させており、
/// 計測ループには `merge-ready-prompt` の実行時間のみが含まれる。
fn bench_prompt_warm_startup(c: &mut Criterion) {
    let env = setup_bench_env();

    assert_warm_startup_threshold(&env);

    let prompt_bin = binary_path("merge-ready-prompt");
    let path = path_env(&env);
    let tmpdir = env.tmp_dir.path().to_owned();
    let repo_dir = env.repo_dir.path().to_owned();

    c.bench_function("merge_ready_prompt_warm_startup", |b| {
        b.iter(|| {
            // 計測対象: merge-ready-prompt の総実行時間（プロセス生成〜終了）
            Command::new(&prompt_bin)
                .env("PATH", &path)
                .env("TMPDIR", &tmpdir)
                .current_dir(&repo_dir)
                .output()
                .expect("merge-ready-prompt failed")
        });
    });
}

criterion_group!(benches, bench_prompt_warm_startup);
criterion_main!(benches);
