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
    /// `repo_dir/.git/HEAD` に書き込んだブランチ名
    pub branch: String,
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

    /// FNV-1a ハッシュで `repo_id` を計算する（`infra::repo_id::path_to_id` と同じアルゴリズム）。
    ///
    /// キーは `"<toplevel>\0<branch>"` の形式。
    /// macOS では `/var/folders/...` が `/private/var/folders/...` のシンボリックリンクのため、
    /// `std::env::current_dir()` が返す正規化パスと一致させるために `canonicalize()` を使用する。
    pub fn repo_id(&self) -> String {
        let canonical = self
            .repo_dir
            .path()
            .canonicalize()
            .unwrap_or_else(|_| self.repo_dir.path().to_path_buf());
        let input = format!("{}\0{}", canonical.display(), self.branch);
        let mut hash: u64 = 14_695_981_039_346_656_037;
        for byte in input.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1_099_511_628_211);
        }
        format!("{hash:016x}")
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
            branch: DEFAULT_BRANCH.to_owned(),
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
            branch: DEFAULT_BRANCH.to_owned(),
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
            branch: DEFAULT_BRANCH.to_owned(),
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
            branch: DEFAULT_BRANCH.to_owned(),
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
            branch: DEFAULT_BRANCH.to_owned(),
        }
    }

    /// `gh` バイナリが `PATH` に存在しないシナリオ（`home_dir` / `repo_dir` は用意する）
    pub fn without_gh() -> Self {
        let (bin_dir, home_dir, repo_dir) = Self::setup_with_git();
        Self {
            bin_dir,
            home_dir,
            repo_dir,
            branch: DEFAULT_BRANCH.to_owned(),
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
            branch: DEFAULT_BRANCH.to_owned(),
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

fn write_executable(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::write(path, content).expect("failed to write script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("failed to chmod script");
}
