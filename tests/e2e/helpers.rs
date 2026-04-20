//! テスト環境ヘルパー
//!
//! 各テストが独立した `bin_dir`（`fake gh`）と `home_dir` / `repo_dir` を持つ。
//! テストを並列実行してもキャッシュやエラーログが競合しない。
//!
//! `repo_id` は `.git` ディレクトリを直接読み取って生成されるため、
//! バイナリの実行ディレクトリを `repo_dir` に設定することで再現性を確保する。

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::{TempDir, tempdir};

/// テスト実行環境を完全に隔離するヘルパー。
pub struct TestEnv {
    /// `fake gh` を配置する一時ディレクトリ
    pub bin_dir: TempDir,
    /// 隔離された `HOME` 兼 `TMPDIR`（キャッシュ・ロックファイルの書き込み先）
    pub home_dir: TempDir,
    /// バイナリを実行するワーキングディレクトリ（`.git/HEAD` を持つ偽リポジトリ）
    /// `None` = `.git` のない空ディレクトリ（git リポジトリ外シナリオ）
    pub repo_dir: TempDir,
}

/// テスト用の固定ブランチ名
const DEFAULT_BRANCH: &str = "main";

impl TestEnv {
    /// `bin_dir` / `home_dir` と最小限の `.git` 構造を持つ `repo_dir` を生成する共通初期化処理。
    fn setup_with_git() -> (TempDir, TempDir, TempDir) {
        let bin_dir = tempdir().expect("failed to create bin_dir");
        let home_dir = tempdir().expect("failed to create home_dir");
        let repo_dir = tempdir().expect("failed to create repo_dir");

        // 最小限の .git 構造（HEAD のみ）
        let git_dir = repo_dir.path().join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");
        fs::write(
            git_dir.join("HEAD"),
            format!("ref: refs/heads/{DEFAULT_BRANCH}\n"),
        )
        .expect("write HEAD");

        (bin_dir, home_dir, repo_dir)
    }

    /// `.git` のない空のワーキングディレクトリを生成する（git リポジトリ外シナリオ用）。
    fn setup_without_git() -> (TempDir, TempDir, TempDir) {
        let bin_dir = tempdir().expect("failed to create bin_dir");
        let home_dir = tempdir().expect("failed to create home_dir");
        let repo_dir = tempdir().expect("failed to create repo_dir");
        // repo_dir に .git を作らない
        (bin_dir, home_dir, repo_dir)
    }

    /// 正常系: `pr view` / `pr checks` それぞれの JSON を返す `fake gh` を配置する。
    ///
    /// `pr_checks_json` が `None` の場合、`gh pr checks` が呼ばれると `exit 1` を返す。
    pub fn new(pr_view_json: &str, pr_checks_json: Option<&str>) -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();

        let checks_block = match pr_checks_json {
            Some(j) => format!("printf '%s' '{j}'\n"),
            None => "printf 'unexpected pr checks call' >&2\nexit 1\n".to_string(),
        };

        let script = format!(
            "#!/bin/sh\n\
             case \"$*\" in\n\
               *'pr view'*)\n\
                 printf '%s' '{pr_view_json}'\n\
                 ;;\n\
               *'pr checks'*)\n\
                 {checks_block}\
                 ;;\n\
               *'api'*'compare'*)\n\
                 printf '{{\"behind_by\":0}}'\n\
                 ;;\n\
               *)\n\
                 printf 'unknown gh command: %s' \"$*\" >&2\n\
                 exit 127\n\
                 ;;\n\
             esac\n"
        );

        write_executable(bin_dir.path().join("gh"), &script);
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// compare API が `behind_by` を返すシナリオ用の `fake gh` を配置する。
    ///
    /// `pr view` JSON には `baseRefName` / `headRefName` を含めること。
    pub fn with_behind_by(
        pr_view_json: &str,
        pr_checks_json: Option<&str>,
        behind_by: u64,
    ) -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();

        let checks_block = match pr_checks_json {
            Some(j) => format!("printf '%s' '{j}'\n"),
            None => "printf 'unexpected pr checks call' >&2\nexit 1\n".to_string(),
        };

        let script = format!(
            "#!/bin/sh\n\
             case \"$*\" in\n\
               *'pr view'*)\n\
                 printf '%s' '{pr_view_json}'\n\
                 ;;\n\
               *'pr checks'*)\n\
                 {checks_block}\
                 ;;\n\
               *'repo view'*)\n\
                 printf '{{\"nameWithOwner\":\"owner/repo\"}}'\n\
                 ;;\n\
               *'api'*'compare'*)\n\
                 printf '{{\"behind_by\":{behind_by}}}'\n\
                 ;;\n\
               *)\n\
                 printf 'unknown gh command: %s' \"$*\" >&2\n\
                 exit 127\n\
                 ;;\n\
             esac\n"
        );

        write_executable(bin_dir.path().join("gh"), &script);
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// Compare API がエラーを返すシナリオ用の `fake gh` を配置する。
    ///
    /// `pr view` JSON には `baseRefName` / `headRefName` を含めること（API を呼ぶため）。
    /// `repo view` は正常に応答し、`api compare` のみ `exit 1` で失敗する。
    pub fn with_compare_error(pr_view_json: &str, pr_checks_json: Option<&str>) -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();

        let checks_block = match pr_checks_json {
            Some(j) => format!("printf '%s' '{j}'\n"),
            None => "printf 'unexpected pr checks call' >&2\nexit 1\n".to_string(),
        };

        let script = format!(
            "#!/bin/sh\n\
             case \"$*\" in\n\
               *'pr view'*)\n\
                 printf '%s' '{pr_view_json}'\n\
                 ;;\n\
               *'pr checks'*)\n\
                 {checks_block}\
                 ;;\n\
               *'repo view'*)\n\
                 printf '{{\"nameWithOwner\":\"owner/repo\"}}'\n\
                 ;;\n\
               *'api'*'compare'*)\n\
                 printf 'API error' >&2\n\
                 exit 1\n\
                 ;;\n\
               *)\n\
                 printf 'unknown gh command: %s' \"$*\" >&2\n\
                 exit 127\n\
                 ;;\n\
             esac\n"
        );

        write_executable(bin_dir.path().join("gh"), &script);
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// エラー系: 指定した `exit_code` と `stderr` メッセージを返す `fake gh` を配置する。
    pub fn with_error(stderr_msg: &str, exit_code: u8) -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();
        let script = format!("#!/bin/sh\nprintf '%s' '{stderr_msg}' >&2\nexit {exit_code}\n");
        write_executable(bin_dir.path().join("gh"), &script);
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// CI 未設定シナリオ: `gh pr view` は成功するが `gh pr checks` が
    /// `"no checks reported"` で `exit 1` を返す。
    pub fn with_no_ci_checks(pr_view_json: &str) -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();
        let script = format!(
            "#!/bin/sh\n\
             case \"$*\" in\n\
               *'pr view'*)\n\
                 printf '%s' '{pr_view_json}'\n\
                 ;;\n\
               *'pr checks'*)\n\
                 printf \"%s\" \"no checks reported on the 'test-branch' branch\" >&2\n\
                 exit 1\n\
                 ;;\n\
               *)\n\
                 printf 'unknown gh command: %s' \"$*\" >&2\n\
                 exit 127\n\
                 ;;\n\
             esac\n"
        );
        write_executable(bin_dir.path().join("gh"), &script);
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// `gh` バイナリが `PATH` に存在しないシナリオ（`home_dir` / `repo_dir` は用意する）
    pub fn without_gh() -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// git リポジトリ外シナリオ（`.git` のない空ディレクトリで実行）
    pub fn without_git_remote() -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_without_git();

        // gh は呼ばれるべきでない
        let gh_script = "#!/bin/sh\necho 'gh should not be called' >&2\nexit 1\n";
        write_executable(bin_dir.path().join("gh"), gh_script);

        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// `gh` バイナリが無期限にハングするシナリオ（タイムアウト検証用）
    pub fn with_hanging_gh() -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();
        write_executable(bin_dir.path().join("gh"), "#!/bin/sh\nsleep 9999\n");
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// `PATH` 文字列を返す（`bin_dir` を先頭に追加）
    pub fn path_env(&self) -> String {
        format!("{}:/bin:/usr/bin", self.bin_dir.path().display())
    }

    /// 隔離された `HOME` パスを返す
    pub fn home(&self) -> &Path {
        self.home_dir.path()
    }

    /// `Command` に `PATH` / `HOME` / `TMPDIR` / `current_dir` をまとめて設定する（キャッシュ無効）
    ///
    /// サブコマンドと `--no-cache` は呼び出し元が追加すること。
    pub fn apply(&self, cmd: &mut assert_cmd::Command) {
        cmd.env("PATH", self.path_env());
        cmd.env("HOME", self.home());
        cmd.env("TMPDIR", self.home());
        cmd.env("XDG_CONFIG_HOME", self.home().join(".config"));
        cmd.current_dir(self.repo_dir.path());
    }

    /// `Command` に `PATH` / `HOME` / `TMPDIR` / `current_dir` / `prompt` サブコマンドをまとめて設定する（キャッシュ有効）
    pub fn apply_with_cache(&self, cmd: &mut assert_cmd::Command) {
        cmd.env("PATH", self.path_env());
        cmd.env("HOME", self.home());
        cmd.env("TMPDIR", self.home());
        cmd.current_dir(self.repo_dir.path());
        cmd.arg("prompt");
    }

    /// `~/.config/merge-ready.toml` に TOML 設定を書き込む。
    pub fn write_config(&self, toml_content: &str) {
        let config_dir = self.home_dir.path().join(".config");
        fs::create_dir_all(&config_dir).expect("create .config");
        fs::write(config_dir.join("merge-ready.toml"), toml_content)
            .expect("write merge-ready.toml");
    }

    /// `bin_dir` に fake editor スクリプトを配置する。
    /// 呼ばれたファイルパス（第一引数）を `home_dir/editor_log.txt` に書き出す。
    /// 戻り値: `(editor_path, log_path)`
    pub fn setup_fake_editor(&self) -> (std::path::PathBuf, std::path::PathBuf) {
        let editor_path = self.bin_dir.path().join("fake_editor");
        let log_path = self.home_dir.path().join("editor_log.txt");
        let script = format!(
            "#!/bin/sh\nprintf '%s' \"$1\" > \"{}\"\n",
            log_path.display()
        );
        write_executable(&editor_path, &script);
        (editor_path, log_path)
    }

    /// `bin_dir` に常に失敗（exit 1）する fake editor スクリプトを配置する。
    /// 戻り値: `editor_path`
    pub fn setup_failing_editor(&self) -> std::path::PathBuf {
        let editor_path = self.bin_dir.path().join("fail_editor");
        write_executable(&editor_path, "#!/bin/sh\nexit 1\n");
        editor_path
    }

    /// `bin_dir/vi` に fake vi スクリプトを配置する（$PATH 経由でフォールバック検証用）。
    /// 呼ばれたファイルパスを `home_dir/vi_log.txt` に書き出す。
    /// 戻り値: `log_path`
    pub fn setup_fake_vi(&self) -> std::path::PathBuf {
        let vi_path = self.bin_dir.path().join("vi");
        let log_path = self.home_dir.path().join("vi_log.txt");
        let script = format!(
            "#!/bin/sh\nprintf '%s' \"$1\" > \"{}\"\n",
            log_path.display()
        );
        write_executable(&vi_path, &script);
        log_path
    }
}

/// `status_cache::infrastructure::paths::dir_name()` と同一のロジック。
///
/// macOS: `"merge-ready"`、Linux: `"merge-ready-{uid}"`
fn daemon_dir_name() -> String {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata("/proc/self") {
            return format!("merge-ready-{}", meta.uid());
        }
    }
    "merge-ready".to_owned()
}

fn write_executable(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::write(path, content).expect("failed to write script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("failed to chmod script");
}

/// daemon プロセスを管理するテストヘルパー。
///
/// socket ファイルの出現をポーリングして起動完了を検知する（固定 sleep は使わない）。
/// Drop 時に daemon を停止する。
pub struct DaemonHandle {
    process: std::process::Child,
    tmpdir: std::path::PathBuf,
}

impl DaemonHandle {
    /// daemon を起動し、socket が出現するまで最大 2000ms ポーリングする。
    #[must_use]
    pub fn start(env: &TestEnv) -> Self {
        Self::start_with_env(env, &[])
    }

    /// 追加の環境変数を指定して daemon を起動する。
    #[must_use]
    pub fn start_with_env(env: &TestEnv, extra_envs: &[(&str, &str)]) -> Self {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");

        let mut cmd = std::process::Command::new(&bin);
        cmd.args(["daemon", "start"])
            .env("PATH", env.path_env())
            .env("HOME", env.home())
            .env("TMPDIR", env.home())
            .env("XDG_CONFIG_HOME", env.home().join(".config"))
            .current_dir(env.repo_dir.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        for (k, v) in extra_envs {
            cmd.env(k, v);
        }
        let child = cmd.spawn().expect("daemon spawn failed");

        let socket = env
            .home_dir
            .path()
            .join(daemon_dir_name())
            .join("daemon.sock");
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2000);
        while std::time::Instant::now() < deadline {
            if socket.exists() {
                return DaemonHandle {
                    process: child,
                    tmpdir: env.home_dir.path().to_path_buf(),
                };
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("daemon did not start within 2000ms");
    }

    /// キャッシュに有効な値が入るまで最大 `max_ms` ミリ秒ポーリングする。
    pub fn wait_for_cache(env: &TestEnv, max_ms: u64) {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
        loop {
            let out = std::process::Command::new(&bin)
                .arg("prompt")
                .env("PATH", env.path_env())
                .env("HOME", env.home())
                .env("TMPDIR", env.home())
                .current_dir(env.repo_dir.path())
                .output()
                .expect("prompt failed");
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout != "? loading" {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "cache not populated within {max_ms}ms"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
        let _ = std::process::Command::new(&bin)
            .args(["daemon", "stop"])
            .env("TMPDIR", &self.tmpdir)
            .output();
        let _ = self.process.kill();
    }
}

/// 複数リポジトリが同一 daemon を共有するシナリオ用の環境。
///
/// `fake gh` は `$PWD/.gh_pr_view.json` を読み取り応答するため、
/// daemon が各エントリの cwd で `gh` を実行するかどうかをそのまま検証できる。
pub struct MultiRepoEnv {
    /// 共有 `fake gh` を配置するディレクトリ
    pub bin_dir: TempDir,
    /// 共有 `HOME` / `TMPDIR`（daemon socket の置き場）
    pub home_dir: TempDir,
    /// リポジトリ A のワーキングディレクトリ
    pub repo_a: TempDir,
    /// リポジトリ B のワーキングディレクトリ
    pub repo_b: TempDir,
}

impl MultiRepoEnv {
    /// `pr_view_a` / `pr_view_b` をそれぞれの repo_dir に仕込み、
    /// `$PWD` ベースで応答する共有 `fake gh` をセットアップする。
    pub fn new(pr_view_a: &str, pr_view_b: &str) -> Self {
        let bin_dir = tempdir().expect("bin_dir");
        let home_dir = tempdir().expect("home_dir");
        let repo_a = tempdir().expect("repo_a");
        let repo_b = tempdir().expect("repo_b");

        // fake gh: $PWD/.gh_pr_view.json を読み取って応答する
        let gh_script = "#!/bin/sh\n\
            case \"$*\" in\n\
              *'pr view'*)\n\
                cat \"$PWD/.gh_pr_view.json\"\n\
                ;;\n\
              *'pr checks'*)\n\
                printf '[{\"bucket\":\"pass\",\"state\":\"SUCCESS\"}]'\n\
                ;;\n\
              *'api'*'compare'*)\n\
                printf '{\"behind_by\":0}'\n\
                ;;\n\
              *)\n\
                printf 'unknown gh command: %s' \"$*\" >&2\n\
                exit 127\n\
                ;;\n\
            esac\n";
        write_executable(bin_dir.path().join("gh"), gh_script);

        for (repo, json) in [(&repo_a, pr_view_a), (&repo_b, pr_view_b)] {
            let git_dir = repo.path().join(".git");
            fs::create_dir_all(&git_dir).expect("create .git");
            fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
            fs::write(repo.path().join(".gh_pr_view.json"), json).expect("write response json");
        }

        Self {
            bin_dir,
            home_dir,
            repo_a,
            repo_b,
        }
    }

    pub fn path_env(&self) -> String {
        format!("{}:/bin:/usr/bin", self.bin_dir.path().display())
    }

    pub fn home(&self) -> &Path {
        self.home_dir.path()
    }

    /// daemon を `repo_a` の cwd で起動し、socket 出現まで最大 2000ms 待つ。
    pub fn start_daemon(&self) -> DaemonHandle {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
        let child = std::process::Command::new(&bin)
            .args(["daemon", "start"])
            .env("PATH", self.path_env())
            .env("HOME", self.home())
            .env("TMPDIR", self.home())
            .current_dir(self.repo_a.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("daemon spawn failed");

        let socket = self.home().join(daemon_dir_name()).join("daemon.sock");
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2000);
        while std::time::Instant::now() < deadline {
            if socket.exists() {
                return DaemonHandle {
                    process: child,
                    tmpdir: self.home().to_path_buf(),
                };
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("daemon did not start within 2000ms");
    }

    /// `repo_dir` の `prompt` 出力が `"? loading"` でなくなるまで最大 `max_ms` ms 待つ。
    pub fn wait_for_cache_in(&self, repo_dir: &TempDir, max_ms: u64) {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
        loop {
            let out = std::process::Command::new(&bin)
                .arg("prompt")
                .env("PATH", self.path_env())
                .env("HOME", self.home())
                .env("TMPDIR", self.home())
                .current_dir(repo_dir.path())
                .output()
                .expect("prompt failed");
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            if stdout != "? loading" {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "cache not populated within {max_ms}ms"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    /// `repo_dir` から `prompt` を実行してその出力を返す。
    pub fn prompt_output(&self, repo_dir: &TempDir) -> String {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
        let out = std::process::Command::new(&bin)
            .arg("prompt")
            .env("PATH", self.path_env())
            .env("HOME", self.home())
            .env("TMPDIR", self.home())
            .current_dir(repo_dir.path())
            .output()
            .expect("prompt failed");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
}
