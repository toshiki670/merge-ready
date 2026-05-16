//! テスト実行環境ヘルパー
//!
//! 各テストが独立した `bin`（`fake gh`）と `home_tmp` / `repo` を持つ。
//! テストを並列実行してもキャッシュやエラーログが競合しない。

use std::fs;
use std::path::Path;
use tempfile::{TempDir, tempdir};

use super::{apply_coverage_env_assert, write_executable};

/// `.git` を持つ一時ディレクトリ群を生成する。fixture モジュールからの呼び出し用。
pub(crate) fn setup_git_dirs(branch: &str) -> (TempDir, TempDir, TempDir) {
    let bin = tempdir().expect("failed to create bin");
    let home_tmp = tempdir().expect("failed to create home_tmp");
    let repo = tempdir().expect("failed to create repo");

    let git_dir = repo.path().join(".git");
    fs::create_dir_all(git_dir.join("objects")).expect("create .git/objects");
    fs::create_dir_all(git_dir.join("refs")).expect("create .git/refs");
    fs::write(git_dir.join("HEAD"), format!("ref: refs/heads/{branch}\n")).expect("write HEAD");
    fs::write(
        git_dir.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n",
    )
    .expect("write config");

    (bin, home_tmp, repo)
}

/// `.git` のない空のディレクトリ群を生成する。fixture モジュールからの呼び出し用。
pub(crate) fn setup_empty_dirs() -> (TempDir, TempDir, TempDir) {
    let bin = tempdir().expect("failed to create bin");
    let home_tmp = tempdir().expect("failed to create home_tmp");
    let repo = tempdir().expect("failed to create repo");
    (bin, home_tmp, repo)
}

/// テスト実行環境を完全に隔離するヘルパー。
pub struct TestEnv {
    /// `fake gh` を配置する一時ディレクトリ
    pub bin: TempDir,
    /// 隔離された `HOME` 兼 `TMPDIR`（キャッシュ・ロックファイルの書き込み先）
    pub home_tmp: TempDir,
    /// バイナリを実行するワーキングディレクトリ（`.git/HEAD` を持つ偽リポジトリ）
    pub repo: TempDir,
}

impl TestEnv {
    /// 正常系: `pr list` / `pr checks` それぞれの JSON を返す `fake gh` を配置する。
    pub fn new(pr_view_json: &str, pr_checks_json: Option<&str>) -> Self {
        let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");

        let checks_block = match pr_checks_json {
            Some(j) => format!("printf '%s' '{j}'\n"),
            None => "printf 'unexpected pr checks call' >&2\nexit 1\n".to_string(),
        };

        let inner = pr_view_json.strip_prefix('{').unwrap_or(pr_view_json);
        let pr_list_json = format!(r#"[{{"number":1,{inner}]"#);

        let script = format!(
            "#!/bin/sh\n\
             case \"$*\" in\n\
               *'pr list'*)\n\
                 printf '%s' '{pr_list_json}'\n\
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

        write_executable(bin.path().join("gh"), &script);
        Self {
            bin,
            home_tmp,
            repo,
        }
    }

    /// PR なしシナリオ: フィーチャーブランチに PR が存在しない場合を模倣する。
    ///
    /// 現在ブランチ: `feat/my-feature`（デフォルトブランチではない）
    /// `gh pr list` → 空配列 `[]` を返す
    /// `gh repo view --json defaultBranchRef` → main をデフォルトブランチとして返す
    pub fn with_no_pr() -> Self {
        let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
        let script = "#!/bin/sh\n\
                      case \"$*\" in\n\
                        *'pr list'*)\n\
                          printf '[]'\n\
                          ;;\n\
                        *'repo view'*'defaultBranchRef'*)\n\
                          printf '{\"defaultBranchRef\":{\"name\":\"main\"}}'\n\
                          ;;\n\
                        *)\n\
                          printf 'unknown gh command: %s' \"$*\" >&2\n\
                          exit 127\n\
                          ;;\n\
                      esac\n";
        write_executable(bin.path().join("gh"), script);
        Self {
            bin,
            home_tmp,
            repo,
        }
    }

    /// エラー系: 指定した `exit_code` と `stderr` メッセージを返す `fake gh` を配置する。
    pub fn with_error(stderr_msg: &str, exit_code: u8) -> Self {
        let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
        let script = format!("#!/bin/sh\nprintf '%s' '{stderr_msg}' >&2\nexit {exit_code}\n");
        write_executable(bin.path().join("gh"), &script);
        Self {
            bin,
            home_tmp,
            repo,
        }
    }

    /// `gh` バイナリが `PATH` に存在しないシナリオ
    pub fn without_gh() -> Self {
        let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
        Self {
            bin,
            home_tmp,
            repo,
        }
    }

    /// 複数 PR シナリオ: `pr_list_json` に複数エントリを含む JSON 配列を直接指定する。
    ///
    /// `pr_list_json` は `[{...}, {...}]` 形式の完全な JSON 配列。
    /// `gh pr checks` はすべての PR 番号に対して同じ `pr_checks_json` を返す。
    pub fn with_pr_list(pr_list_json: &str, pr_checks_json: Option<&str>) -> Self {
        let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");

        let checks_block = match pr_checks_json {
            Some(j) => format!("printf '%s' '{j}'\n"),
            None => "printf 'unexpected pr checks call' >&2\nexit 1\n".to_string(),
        };

        let script = format!(
            "#!/bin/sh\n\
             case \"$*\" in\n\
               *'pr list'*)\n\
                 printf '%s' '{pr_list_json}'\n\
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

        write_executable(bin.path().join("gh"), &script);
        Self {
            bin,
            home_tmp,
            repo,
        }
    }

    /// `PATH` 文字列を返す（`bin` を先頭に追加）
    pub fn path_env(&self) -> String {
        format!("{}:/bin:/usr/bin", self.bin.path().display())
    }

    /// 隔離された `HOME` パスを返す
    pub fn home(&self) -> &Path {
        self.home_tmp.path()
    }

    /// `Command` に `PATH` / `HOME` / `TMPDIR` / `MERGE_READY_BASE_DIR` / `XDG_CONFIG_HOME` / `current_dir` を設定する。
    pub fn apply(&self, cmd: &mut assert_cmd::Command) {
        cmd.env("PATH", self.path_env());
        cmd.env("HOME", self.home());
        cmd.env("TMPDIR", self.home());
        cmd.env("MERGE_READY_BASE_DIR", self.home());
        cmd.env("XDG_CONFIG_HOME", self.home().join(".config"));
        cmd.current_dir(self.repo.path());
        apply_coverage_env_assert(cmd);
    }

    /// `Command` に環境変数を設定する（`merge-ready-prompt` バイナリ用）。
    ///
    /// 呼び出し元は `Command::cargo_bin("merge-ready-prompt")` で作成したコマンドを渡す。
    pub fn apply_with_cache(&self, cmd: &mut assert_cmd::Command) {
        cmd.env("PATH", self.path_env());
        cmd.env("HOME", self.home());
        cmd.env("TMPDIR", self.home());
        cmd.env("MERGE_READY_BASE_DIR", self.home());
        cmd.current_dir(self.repo.path());
        apply_coverage_env_assert(cmd);
    }

    /// `~/.config/merge-ready.toml` に TOML 設定を書き込む。
    pub fn write_config(&self, toml_content: &str) {
        let config_dir = self.home_tmp.path().join(".config");
        fs::create_dir_all(&config_dir).expect("create .config");
        fs::write(config_dir.join("merge-ready.toml"), toml_content)
            .expect("write merge-ready.toml");
    }

    /// `bin` に fake editor スクリプトを配置する。
    pub fn setup_fake_editor(&self) -> (std::path::PathBuf, std::path::PathBuf) {
        let editor_path = self.bin.path().join("fake_editor");
        let log_path = self.home_tmp.path().join("editor_log.txt");
        let script = format!(
            "#!/bin/sh\nprintf '%s' \"$1\" > \"{}\"\n",
            log_path.display()
        );
        write_executable(&editor_path, &script);
        (editor_path, log_path)
    }

    /// `bin` に常に失敗する fake editor スクリプトを配置する。
    pub fn setup_failing_editor(&self) -> std::path::PathBuf {
        let editor_path = self.bin.path().join("fail_editor");
        write_executable(&editor_path, "#!/bin/sh\nexit 1\n");
        editor_path
    }

    /// `bin/vi` に fake vi スクリプトを配置する（`$PATH` 経由でフォールバック検証用）。
    pub fn setup_fake_vi(&self) -> std::path::PathBuf {
        let vi_path = self.bin.path().join("vi");
        let log_path = self.home_tmp.path().join("vi_log.txt");
        let script = format!(
            "#!/bin/sh\nprintf '%s' \"$1\" > \"{}\"\n",
            log_path.display()
        );
        write_executable(&vi_path, &script);
        log_path
    }
}
