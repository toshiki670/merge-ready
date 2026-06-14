//! `rate_limit` aware シナリオ用 `fixture`。
//!
//! fake `gh` は `graphql`（refresh 本体）/ `api compare` / `api rate_limit` を扱う。
//! `pr_list_log` は refresh あたり 1 回の `graphql` 呼び出しをカウントする。
//! `api rate_limit` のレスポンスは `remaining_bp`（0..=10000）に応じた残量を返す。

use std::path::PathBuf;

use super::super::helpers::{
    ROLLUP_PASS, TestEnv, graphql_single, setup_git_dirs, write_executable,
};

pub struct RateLimitFixture {
    pub env: TestEnv,
    pub pr_list_log: PathBuf,
    pub rate_limit_log: PathBuf,
}

/// `rate_limit` に応じて静的なスナップショットを返す fake `gh` を構築する。
///
/// - `remaining_bp` = 10000 で「ほぼ満タン」（残量 4999/5000）
/// - `remaining_bp` = 0 で「枯渇」（残量 0/5000）
/// - `reset_offset_secs` は現在時刻＋指定秒を `reset` の unix epoch として返す
pub fn with_rate_limit_response(remaining_bp: u32, reset_offset_secs: u64) -> RateLimitFixture {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let pr_list_log = home_tmp.path().join("pr_list_calls.log");
    let rate_limit_log = home_tmp.path().join("rate_limit_calls.log");
    let pr_list_log_s = pr_list_log.display().to_string();
    let rate_limit_log_s = rate_limit_log.display().to_string();

    let limit: u32 = 5_000;
    let remaining: u32 = (u64::from(remaining_bp.min(10_000)) * u64::from(limit) / 10_000)
        .try_into()
        .unwrap_or(0);

    let graphql_json = graphql_single(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(ROLLUP_PASS),
    );

    // reset は absolute unix epoch を渡したいので、シェルスクリプト内で date を呼ぶ
    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *'api rate_limit'*)\n\
             printf '1' >> \"{rate_limit_log_s}\"\n\
             now=$(date +%s)\n\
             reset=$((now + {reset_offset_secs}))\n\
             printf '{{\"resources\":{{\"core\":{{\"limit\":{limit},\"remaining\":{remaining},\"reset\":%d}},\"graphql\":{{\"limit\":{limit},\"remaining\":{remaining},\"reset\":%d}}}}}}' \"$reset\" \"$reset\"\n\
             exit 0\n\
             ;;\n\
           *graphql*)\n\
             printf '1' >> \"{pr_list_log_s}\"\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf '{{\"behind_by\":0}}'\n\
             ;;\n\
           *)\n\
             printf 'unexpected gh call: %s' \"$*\" >&2\n\
             exit 127\n\
             ;;\n\
         esac\n"
    );
    write_executable(bin.path().join("gh"), &script);

    RateLimitFixture {
        env: TestEnv {
            bin,
            home_tmp,
            repo,
        },
        pr_list_log,
        rate_limit_log,
    }
}

/// Issue #407: 1 回目の `api rate_limit` で **graphql のみ枯渇**させ、core は健全だが
/// reset が遠い未来（now + 3600）を返す fake gh を構築する。2 回目以降は両方フル。
///
/// `graphql_reset_offset_secs` は 1 回目応答の graphql reset (now + offset)。
/// backoff の解除に core 固定の reset を使うバグがあると遠い未来まで再開しないが、
/// ボトルネック（graphql）の reset を使えば短時間で再開する。
pub fn with_graphql_exhausted_core_late_reset(graphql_reset_offset_secs: u64) -> RateLimitFixture {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let pr_list_log = home_tmp.path().join("pr_list_calls.log");
    let rate_limit_log = home_tmp.path().join("rate_limit_calls.log");
    let pr_list_log_s = pr_list_log.display().to_string();
    let rate_limit_log_s = rate_limit_log.display().to_string();
    let rate_limit_counter = home_tmp.path().join("rate_limit_counter");
    let rate_limit_counter_s = rate_limit_counter.display().to_string();

    let limit: u32 = 5_000;
    let graphql_json = graphql_single(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(ROLLUP_PASS),
    );

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *'api rate_limit'*)\n\
             printf '1' >> \"{rate_limit_log_s}\"\n\
             count=$(cat \"{rate_limit_counter_s}\" 2>/dev/null || printf '0')\n\
             count=$((count + 1))\n\
             printf '%d' \"$count\" > \"{rate_limit_counter_s}\"\n\
             now=$(date +%s)\n\
             core_remaining=4999\n\
             core_reset=$((now + 3600))\n\
             if [ \"$count\" -eq 1 ]; then\n\
               graphql_remaining=0\n\
               graphql_reset=$((now + {graphql_reset_offset_secs}))\n\
             else\n\
               graphql_remaining=4999\n\
               graphql_reset=$((now + 3600))\n\
             fi\n\
             printf '{{\"resources\":{{\"core\":{{\"limit\":{limit},\"remaining\":%d,\"reset\":%d}},\"graphql\":{{\"limit\":{limit},\"remaining\":%d,\"reset\":%d}}}}}}' \"$core_remaining\" \"$core_reset\" \"$graphql_remaining\" \"$graphql_reset\"\n\
             exit 0\n\
             ;;\n\
           *graphql*)\n\
             printf '1' >> \"{pr_list_log_s}\"\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf '{{\"behind_by\":0}}'\n\
             ;;\n\
           *)\n\
             printf 'unexpected gh call: %s' \"$*\" >&2\n\
             exit 127\n\
             ;;\n\
         esac\n"
    );
    write_executable(bin.path().join("gh"), &script);

    RateLimitFixture {
        env: TestEnv {
            bin,
            home_tmp,
            repo,
        },
        pr_list_log,
        rate_limit_log,
    }
}

/// 1 回目の `api rate_limit` で枯渇、2 回目以降で残量フルを返す fake gh を構築する。
/// バックオフ → reset 経過後の再開を検証するシナリオで使用する。
///
/// `reset_offset_secs_first` は 1 回目の応答で渡す reset (now + offset)。
pub fn with_rate_limit_exhaust_then_recover(reset_offset_secs_first: u64) -> RateLimitFixture {
    let (bin, home_tmp, repo) = setup_git_dirs("feat/my-feature");
    let pr_list_log = home_tmp.path().join("pr_list_calls.log");
    let rate_limit_log = home_tmp.path().join("rate_limit_calls.log");
    let pr_list_log_s = pr_list_log.display().to_string();
    let rate_limit_log_s = rate_limit_log.display().to_string();
    let rate_limit_counter = home_tmp.path().join("rate_limit_counter");
    let rate_limit_counter_s = rate_limit_counter.display().to_string();

    let limit: u32 = 5_000;
    let graphql_json = graphql_single(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(ROLLUP_PASS),
    );

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           *'api rate_limit'*)\n\
             printf '1' >> \"{rate_limit_log_s}\"\n\
             count=$(cat \"{rate_limit_counter_s}\" 2>/dev/null || printf '0')\n\
             count=$((count + 1))\n\
             printf '%d' \"$count\" > \"{rate_limit_counter_s}\"\n\
             now=$(date +%s)\n\
             if [ \"$count\" -eq 1 ]; then\n\
               remaining=0\n\
               reset=$((now + {reset_offset_secs_first}))\n\
             else\n\
               remaining=4999\n\
               reset=$((now + 3600))\n\
             fi\n\
             printf '{{\"resources\":{{\"core\":{{\"limit\":{limit},\"remaining\":%d,\"reset\":%d}},\"graphql\":{{\"limit\":{limit},\"remaining\":%d,\"reset\":%d}}}}}}' \"$remaining\" \"$reset\" \"$remaining\" \"$reset\"\n\
             exit 0\n\
             ;;\n\
           *graphql*)\n\
             printf '1' >> \"{pr_list_log_s}\"\n\
             printf '%s' '{graphql_json}'\n\
             ;;\n\
           *'api'*'compare'*)\n\
             printf '{{\"behind_by\":0}}'\n\
             ;;\n\
           *)\n\
             printf 'unexpected gh call: %s' \"$*\" >&2\n\
             exit 127\n\
             ;;\n\
         esac\n"
    );
    write_executable(bin.path().join("gh"), &script);

    RateLimitFixture {
        env: TestEnv {
            bin,
            home_tmp,
            repo,
        },
        pr_list_log,
        rate_limit_log,
    }
}
