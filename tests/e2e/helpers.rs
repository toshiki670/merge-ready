//! テスト環境ヘルパー
//!
//! 各テストが独立した `bin_dir`（`fake gh` / `fake git`）と `home_dir` を持つ。
//! テストを並列実行してもキャッシュやエラーログが競合しない。

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::{TempDir, tempdir};

/// テスト実行環境を完全に隔離するヘルパー。
pub struct TestEnv {
    /// `fake gh` / `fake git` を配置する一時ディレクトリ
    pub bin_dir: TempDir,
    /// 隔離された `HOME`（`~/.cache/ci-status/error.log` の書き込み先）
    pub home_dir: TempDir,
}

/// `git` の引数別に固定値を返すフェイクスクリプト
const FAKE_GIT_SCRIPT: &str = r#"#!/bin/sh
case "$*" in
  *'rev-parse --is-inside-work-tree'*)
    echo 'true'; exit 0 ;;
  *'rev-parse --show-toplevel'*)
    echo '/fake/repo'; exit 0 ;;
  *'branch --show-current'*)
    echo 'main'; exit 0 ;;
  *'rev-parse --abbrev-ref HEAD'*)
    echo 'main'; exit 0 ;;
  *)
    exit 0 ;;
esac
"#;

impl TestEnv {
    /// `bin_dir` に `fake git` を配置し、`home_dir` を生成する共通初期化処理
    fn setup() -> (TempDir, TempDir) {
        let bin_dir = tempdir().expect("failed to create bin_dir");
        let home_dir = tempdir().expect("failed to create home_dir");
        write_executable(bin_dir.path().join("git"), FAKE_GIT_SCRIPT);
        (bin_dir, home_dir)
    }

    /// 正常系: `pr view` / `pr checks` それぞれの JSON を返す `fake gh` を配置する。
    ///
    /// `pr_checks_json` が `None` の場合、`gh pr checks` が呼ばれると `exit 1` を返す。
    pub fn new(pr_view_json: &str, pr_checks_json: Option<&str>) -> Self {
        let (bin_dir, home_dir) = Self::setup();

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
               *)\n\
                 printf 'unknown gh command: %s' \"$*\" >&2\n\
                 exit 127\n\
                 ;;\n\
             esac\n"
        );

        write_executable(bin_dir.path().join("gh"), &script);
        Self { bin_dir, home_dir }
    }

    /// エラー系: 指定した `exit_code` と `stderr` メッセージを返す `fake gh` を配置する。
    pub fn with_error(stderr_msg: &str, exit_code: u8) -> Self {
        let (bin_dir, home_dir) = Self::setup();
        let script = format!("#!/bin/sh\nprintf '%s' '{stderr_msg}' >&2\nexit {exit_code}\n");
        write_executable(bin_dir.path().join("gh"), &script);
        Self { bin_dir, home_dir }
    }

    /// `gh` バイナリが `PATH` に存在しないシナリオ（`fake git` と `home_dir` は用意する）
    pub fn without_gh() -> Self {
        let (bin_dir, home_dir) = Self::setup();
        Self { bin_dir, home_dir }
    }

    /// `PATH` 文字列を返す（`bin_dir` を先頭に追加）
    pub fn path_env(&self) -> String {
        format!("{}:/bin:/usr/bin", self.bin_dir.path().display())
    }

    /// 隔離された `HOME` パスを返す
    pub fn home(&self) -> &Path {
        self.home_dir.path()
    }

    /// `Command` に `PATH` / `HOME` をまとめて設定する
    pub fn apply(&self, cmd: &mut assert_cmd::Command) {
        cmd.env("PATH", self.path_env());
        cmd.env("HOME", self.home());
    }
}

fn write_executable(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::write(path, content).expect("failed to write script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("failed to chmod script");
}
