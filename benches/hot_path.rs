//! ホットパスのベンチマーク
//!
//! キャッシュヒットパス（ファイル読み込み → 出力）と
//! キャッシュミスパス（フェイク gh を使った全フロー）の両方を計測する。
//!
//! 使用方法:
//! ```
//! cargo bench --bench hot_path
//! ```

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;

// ── フェイク環境のセットアップ ──────────────────────────────────────────────

struct BenchEnv {
    bin_dir: TempDir,
    home_dir: TempDir,
}

const OPEN_MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// フェイクの gh/git バイナリを含むベンチマーク環境を構築する
fn setup_bench_env() -> BenchEnv {
    let bin_dir = TempDir::new().expect("bin_dir");
    let home_dir = TempDir::new().expect("home_dir");

    // fake git
    let git_script = "#!/bin/sh\n\
        case \"$*\" in\n\
          *'remote get-url origin'*) echo 'https://github.com/bench/repo.git'; exit 0 ;;\n\
          *) exit 0 ;;\n\
        esac\n";
    write_executable(&bin_dir.path().join("git"), git_script);

    // fake gh（即時応答）
    // JSON に `{}` が含まれているため format! ではなく文字列連結を使用
    let gh_script = "#!/bin/sh\ncase \"$*\" in\n  *'pr view'*) printf '%s' '".to_owned()
        + OPEN_MERGE_READY_JSON
        + "' ;;\n  *'pr checks'*) printf '%s' '"
        + CI_PASS_JSON
        + "' ;;\n  *) exit 0 ;;\nesac\n";
    write_executable(&bin_dir.path().join("gh"), &gh_script);

    BenchEnv { bin_dir, home_dir }
}

fn write_executable(path: &PathBuf, content: &str) {
    fs::write(path, content).expect("write");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn path_env(env: &BenchEnv) -> String {
    format!("{}:/bin:/usr/bin", env.bin_dir.path().display())
}

fn binary_path() -> PathBuf {
    let mut path = std::env::current_exe().expect("current_exe");
    // benches/hot_path → target/debug/deps/hot_path-xxx
    // binary は target/debug/merge-ready
    path.pop(); // hot_path-xxx
    path.pop(); // deps
    path.push("merge-ready");
    if !path.exists() {
        // release build の場合
        path.pop();
        path.pop();
        path.push("release");
        path.push("merge-ready");
    }
    path
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// キャッシュヒットパスのベンチマーク（目標: <5ms）
///
/// ファイル読み込み → 文字列返却 のみを計測する。
fn bench_cache_hit(c: &mut Criterion) {
    let env = setup_bench_env();
    let repo_id = "https___github_com_bench_repo_git";
    let cache_dir = env
        .home_dir
        .path()
        .join(".cache")
        .join("merge-ready")
        .join(repo_id);
    fs::create_dir_all(&cache_dir).expect("create cache dir");

    let state_json = format!(
        r#"{{"fetched_at_secs":{now},"output":"✓ merge-ready"}}"#,
        now = now_secs()
    );
    fs::write(cache_dir.join("state.json"), state_json).expect("write state.json");

    let bin = binary_path();
    let path = path_env(&env);
    let home = env.home_dir.path().to_owned();

    c.bench_function("cache_hit", |b| {
        b.iter(|| {
            Command::new(&bin)
                .env("PATH", &path)
                .env("HOME", &home)
                .output()
                .expect("merge-ready failed")
        });
    });
}

/// キャッシュミスパスのベンチマーク（フェイク gh 使用）
///
/// `MERGE_READY_NO_CACHE=1` で従来のフロー（gh 呼び出し）を計測する。
/// ネットワーク遅延はないが、プロセス起動 + JSON パース + ロジックのコストが含まれる。
fn bench_no_cache_direct(c: &mut Criterion) {
    let env = setup_bench_env();
    let bin = binary_path();
    let path = path_env(&env);
    let home = env.home_dir.path().to_owned();

    c.bench_function("no_cache_direct", |b| {
        b.iter(|| {
            Command::new(&bin)
                .env("PATH", &path)
                .env("HOME", &home)
                .env("MERGE_READY_NO_CACHE", "1")
                .output()
                .expect("merge-ready failed")
        });
    });
}

criterion_group!(benches, bench_cache_hit, bench_no_cache_direct);
criterion_main!(benches);
