//! ホットパスのベンチマーク
//!
//! キャッシュヒットパス（ファイル読み込み → 出力）と
//! キャッシュなし直接実行パス（フェイク gh を使った全フロー）の両方を計測する。
//!
//! 使用方法:
//! ```
//! cargo bench --bench hot_path
//! ```

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;

// ── フェイク環境のセットアップ ──────────────────────────────────────────────

struct BenchEnv {
    bin_dir: TempDir,
    tmp_dir: TempDir,
    /// 最小限の `.git` 構造を持つ偽リポジトリルート
    repo_dir: TempDir,
}

const FAKE_BRANCH: &str = "main";
const OPEN_MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// FNV-1a ハッシュで repo_id を生成する（`infra::repo_id::path_to_id` と同じアルゴリズム）
///
/// キーは `"<toplevel>\0<branch>"` の形式で生成する。
fn compute_repo_id(toplevel: &Path, branch: &str) -> String {
    let input = format!("{}\0{}", toplevel.display(), branch);
    let mut hash: u64 = 14_695_981_039_346_656_037;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    format!("{hash:016x}")
}

/// フェイクの gh バイナリと最小限の git リポジトリ構造を含むベンチマーク環境を構築する
fn setup_bench_env() -> BenchEnv {
    let bin_dir = TempDir::new().expect("bin_dir");

    // fake gh（即時応答）
    // JSON に `{}` が含まれているため format! ではなく文字列連結を使用
    let gh_script = "#!/bin/sh\ncase \"$*\" in\n  *'pr view'*) printf '%s' '".to_owned()
        + OPEN_MERGE_READY_JSON
        + "' ;;\n  *'pr checks'*) printf '%s' '"
        + CI_PASS_JSON
        + "' ;;\n  *'api'*'compare'*) printf '{\"behind_by\":0}' ;;\n  *) exit 0 ;;\nesac\n";
    write_executable(&bin_dir.path().join("gh"), &gh_script);

    let tmp_dir = TempDir::new().expect("tmp_dir");

    // 最小限の git リポジトリ構造（.git/HEAD のみ）
    let repo_dir = TempDir::new().expect("repo_dir");
    let git_dir = repo_dir.path().join(".git");
    fs::create_dir_all(&git_dir).expect("create .git");
    fs::write(
        git_dir.join("HEAD"),
        format!("ref: refs/heads/{FAKE_BRANCH}\n"),
    )
    .expect("write HEAD");

    BenchEnv {
        bin_dir,
        tmp_dir,
        repo_dir,
    }
}

fn write_executable(path: &PathBuf, content: &str) {
    fs::write(path, content).expect("write");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn path_env(env: &BenchEnv) -> String {
    format!("{}:/bin:/usr/bin", env.bin_dir.path().display())
}

/// `std::env::temp_dir()` と同じロジックでキャッシュディレクトリのサブディレクトリ名を返す。
///
/// バイナリ側の `infra::tmp_cache_dir::dir_name()` と同一のロジックを複製している。
/// macOS: "merge-ready"、Linux: "merge-ready-{uid}"
fn bench_cache_dir_name() -> String {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata("/proc/self") {
            return format!("merge-ready-{}", meta.uid());
        }
    }
    "merge-ready".to_owned()
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
/// `prompt` サブコマンドでキャッシュファイルを読み込んで返すパスのみを計測する。
fn bench_cache_hit(c: &mut Criterion) {
    let env = setup_bench_env();
    let repo_id = compute_repo_id(env.repo_dir.path(), FAKE_BRANCH);
    let cache_path = env
        .tmp_dir
        .path()
        .join(bench_cache_dir_name())
        .join(format!("{repo_id}.json"));

    fs::create_dir_all(cache_path.parent().unwrap()).expect("create cache dir");
    let state_json = format!(
        r#"{{"fetched_at_secs":{now},"output":"✓ merge-ready"}}"#,
        now = now_secs()
    );
    fs::write(&cache_path, state_json).expect("write cache");

    let bin = binary_path();
    let path = path_env(&env);
    let tmpdir = env.tmp_dir.path().to_owned();
    let repo_dir = env.repo_dir.path().to_owned();

    c.bench_function("cache_hit", |b| {
        b.iter(|| {
            Command::new(&bin)
                .env("PATH", &path)
                .env("TMPDIR", &tmpdir)
                .current_dir(&repo_dir)
                .arg("prompt")
                .output()
                .expect("merge-ready failed")
        });
    });
}

/// キャッシュなし直接実行パスのベンチマーク（フェイク gh 使用）
///
/// `prompt --no-cache` で gh を直接呼ぶフローを計測する。
/// ネットワーク遅延はないが、プロセス起動 + JSON パース + ロジックのコストが含まれる。
fn bench_no_cache_direct(c: &mut Criterion) {
    let env = setup_bench_env();
    let bin = binary_path();
    let path = path_env(&env);
    let tmpdir = env.tmp_dir.path().to_owned();
    let repo_dir = env.repo_dir.path().to_owned();

    c.bench_function("no_cache_direct", |b| {
        b.iter(|| {
            Command::new(&bin)
                .env("PATH", &path)
                .env("TMPDIR", &tmpdir)
                .current_dir(&repo_dir)
                .args(["prompt", "--no-cache"])
                .output()
                .expect("merge-ready failed")
        });
    });
}

criterion_group!(benches, bench_cache_hit, bench_no_cache_direct);
criterion_main!(benches);
