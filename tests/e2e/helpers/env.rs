//! テスト実行環境ヘルパー
//!
//! 各テストが独立した `bin_dir`（`fake gh`）と `home_dir` / `repo_dir` を持つ。
//! テストを並列実行してもキャッシュやエラーログが競合しない。

use std::fs;
use std::path::Path;
use tempfile::{TempDir, tempdir};

use super::write_executable;

/// テスト実行環境を完全に隔離するヘルパー。
pub struct TestEnv {
    /// `fake gh` を配置する一時ディレクトリ
    pub bin_dir: TempDir,
    /// 隔離された `HOME` 兼 `TMPDIR`（キャッシュ・ロックファイルの書き込み先）
    pub home_dir: TempDir,
    /// バイナリを実行するワーキングディレクトリ（`.git/HEAD` を持つ偽リポジトリ）
    pub repo_dir: TempDir,
}

const DEFAULT_BRANCH: &str = "main";

impl TestEnv {
    fn setup_with_git() -> (TempDir, TempDir, TempDir) {
        let bin_dir = tempdir().expect("failed to create bin_dir");
        let home_dir = tempdir().expect("failed to create home_dir");
        let repo_dir = tempdir().expect("failed to create repo_dir");

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
        (bin_dir, home_dir, repo_dir)
    }

    /// 正常系: `pr view` / `pr checks` それぞれの JSON を返す `fake gh` を配置する。
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

    /// compare API がエラーを返すシナリオ用の `fake gh` を配置する。
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

    /// CI 未設定シナリオ: `gh pr checks` が `"no checks reported"` で `exit 1` を返す。
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

    /// `gh` バイナリが `PATH` に存在しないシナリオ
    pub fn without_gh() -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// terminal PR シナリオ（呼び出しカウンタ付き）
    ///
    /// `gh pr view` が closed / merged JSON を返す。カウンタログファイルのパスを返す。
    pub fn with_terminal_pr_call_log(state: &str) -> (Self, std::path::PathBuf) {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();
        let log_path = home_dir.path().join("gh_calls.log");
        let log = log_path.display().to_string();
        let pr_view_json = format!(
            r#"{{"state":"{state}","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}}"#
        );
        let script = format!(
            "#!/bin/sh\n\
             printf '1' >> \"{log}\"\n\
             case \"$*\" in\n\
               *'pr view'*)\n\
                 printf '%s' '{pr_view_json}'\n\
                 ;;\n\
               *)\n\
                 printf 'unexpected gh call: %s' \"$*\" >&2\n\
                 exit 127\n\
                 ;;\n\
             esac\n"
        );
        write_executable(bin_dir.path().join("gh"), &script);
        (
            Self {
                bin_dir,
                home_dir,
                repo_dir,
            },
            log_path,
        )
    }

    /// PR なしシナリオ: `gh pr view` が "no pull requests found" で exit 1 を返す。
    pub fn with_no_pr() -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();
        let script = "#!/bin/sh\nprintf 'no pull requests found' >&2\nexit 1\n";
        write_executable(bin_dir.path().join("gh"), script);
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// PR なしシナリオ（遅延付き）: stale refresh 中の挙動を再現するため gh を長引かせる。
    pub fn with_no_pr_delay_ms(delay_ms: u64) -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();
        let secs = delay_ms / 1000;
        let millis = delay_ms % 1000;
        let sleep_arg = format!("{secs}.{millis:03}");
        let script =
            format!("#!/bin/sh\nsleep {sleep_arg}\nprintf 'no pull requests found' >&2\nexit 1\n");
        write_executable(bin_dir.path().join("gh"), &script);
        Self {
            bin_dir,
            home_dir,
            repo_dir,
        }
    }

    /// git リポジトリ外シナリオ（`.git` のない空ディレクトリで実行）
    pub fn without_git_remote() -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_without_git();
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

    /// `Command` に `PATH` / `HOME` / `TMPDIR` / `XDG_CONFIG_HOME` / `current_dir` を設定する。
    pub fn apply(&self, cmd: &mut assert_cmd::Command) {
        cmd.env("PATH", self.path_env());
        cmd.env("HOME", self.home());
        cmd.env("TMPDIR", self.home());
        cmd.env("XDG_CONFIG_HOME", self.home().join(".config"));
        cmd.current_dir(self.repo_dir.path());
    }

    /// `Command` に環境変数を設定する（`merge-ready-prompt` バイナリ用）。
    ///
    /// 呼び出し元は `Command::cargo_bin("merge-ready-prompt")` で作成したコマンドを渡す。
    pub fn apply_with_cache(&self, cmd: &mut assert_cmd::Command) {
        cmd.env("PATH", self.path_env());
        cmd.env("HOME", self.home());
        cmd.env("TMPDIR", self.home());
        cmd.current_dir(self.repo_dir.path());
    }

    /// `~/.config/merge-ready.toml` に TOML 設定を書き込む。
    pub fn write_config(&self, toml_content: &str) {
        let config_dir = self.home_dir.path().join(".config");
        fs::create_dir_all(&config_dir).expect("create .config");
        fs::write(config_dir.join("merge-ready.toml"), toml_content)
            .expect("write merge-ready.toml");
    }

    /// `bin_dir` に fake editor スクリプトを配置する。
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

    /// `bin_dir` に常に失敗する fake editor スクリプトを配置する。
    pub fn setup_failing_editor(&self) -> std::path::PathBuf {
        let editor_path = self.bin_dir.path().join("fail_editor");
        write_executable(&editor_path, "#!/bin/sh\nexit 1\n");
        editor_path
    }

    /// `bin_dir/vi` に fake vi スクリプトを配置する（`$PATH` 経由でフォールバック検証用）。
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
