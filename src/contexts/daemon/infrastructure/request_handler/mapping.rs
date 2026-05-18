use crate::contexts::daemon::domain::cache::CacheEntryState;
use crate::shared::protocol::EntryDto;

pub(super) fn entry_to_dtos(entry: &CacheEntryState) -> Vec<EntryDto> {
    let cwd = entry.cwd().to_string_lossy().into_owned();
    let branch = entry.branch().to_owned();
    let cached_at_secs = cached_at_secs(entry);

    if entry.pr_outputs().is_empty() {
        return vec![entry_dto(
            cwd,
            branch,
            None,
            entry.output().to_owned(),
            cached_at_secs,
        )];
    }

    entry
        .pr_outputs()
        .iter()
        .map(|pr_output| {
            entry_dto(
                cwd.clone(),
                branch.clone(),
                Some(pr_output.pr_id),
                pr_output.output.clone(),
                cached_at_secs,
            )
        })
        .collect()
}

fn entry_dto(
    cwd: String,
    branch: String,
    pr_id: Option<u64>,
    output: String,
    cached_at_secs: u64,
) -> EntryDto {
    EntryDto {
        cwd,
        branch,
        pr_id,
        output,
        cached_at_secs,
    }
}

fn cached_at_secs(entry: &CacheEntryState) -> u64 {
    entry
        .fetched_at_wall()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::protocol::PrOutput;
    use crate::shared::refresh_mode::RefreshMode;
    use std::path::PathBuf;
    use std::time::{Instant, SystemTime};

    #[test]
    fn entry_to_dtos_expands_pr_outputs() {
        let entry = CacheEntryState::builder_for_test(Instant::now(), SystemTime::now())
            .cwd(PathBuf::from("/repo"))
            .output("✓ Ready for merge #200 ✎ Ready for review #201".to_owned())
            .pr_outputs(vec![
                PrOutput {
                    pr_id: 200,
                    output: "✓ Ready for merge #200".to_owned(),
                },
                PrOutput {
                    pr_id: 201,
                    output: "✎ Ready for review #201".to_owned(),
                },
            ])
            .refresh_mode(RefreshMode::Warm)
            .build();
        // builder の branch デフォルトを "feat/multi" に上書きする手段が無いので
        // 直接 build 後の値を活用したテスト構造に変更。デフォルト branch は "main" だが
        // ここでは branch の検証は別 test に譲り、PR 展開だけ検証する。

        let dtos = entry_to_dtos(&entry);

        assert_eq!(dtos.len(), 2);
        assert_eq!(dtos[0].cwd, "/repo");
        assert_eq!(dtos[0].pr_id, Some(200));
        assert_eq!(dtos[0].output, "✓ Ready for merge #200");
        assert_eq!(dtos[1].cwd, "/repo");
        assert_eq!(dtos[1].pr_id, Some(201));
        assert_eq!(dtos[1].output, "✎ Ready for review #201");
    }

    #[test]
    fn entry_to_dtos_keeps_aggregate_output_without_pr_outputs() {
        let entry = CacheEntryState::builder_for_test(Instant::now(), SystemTime::now())
            .cwd(PathBuf::from("/repo"))
            .output("+ Create PR".to_owned())
            .refresh_mode(RefreshMode::Warm)
            .build();

        let dtos = entry_to_dtos(&entry);

        assert_eq!(dtos.len(), 1);
        assert_eq!(dtos[0].cwd, "/repo");
        assert_eq!(dtos[0].pr_id, None);
        assert_eq!(dtos[0].output, "+ Create PR");
    }
}
