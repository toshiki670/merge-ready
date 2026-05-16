//! cargo-llvm-cov 配下で実行されるとき、subprocess に `LLVM_PROFILE_FILE` を
//! `%p` 付きで明示伝播するためのヘルパ。
//!
//! `std::process::Command` は環境変数を継承するため伝播自体は通常通り動くが、
//! E2E テストは並列に多数の `merge-ready` プロセスを起動する。`%p` を含めない
//! pattern だと profraw が衝突しうるため、ここで pattern を補正してから明示的に
//! 渡し直す。cargo-llvm-cov 配下でないとき (env 未設定) は何もしない。

/// `std::process::Command` 用。
pub(crate) fn apply_coverage_env(cmd: &mut std::process::Command) {
    if let Ok(value) = std::env::var("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", ensure_per_process_pattern(&value));
    }
}

/// `assert_cmd::Command` 用。
pub(crate) fn apply_coverage_env_assert(cmd: &mut assert_cmd::Command) {
    if let Ok(value) = std::env::var("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", ensure_per_process_pattern(&value));
    }
}

fn ensure_per_process_pattern(path: &str) -> String {
    if path.contains("%p") {
        path.to_owned()
    } else if let Some(stem) = path.strip_suffix(".profraw") {
        format!("{stem}-%p.profraw")
    } else {
        format!("{path}-%p")
    }
}
