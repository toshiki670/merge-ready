use super::super::protocol::{EntryDto, PrOutputDto, RefreshModeDto};
use crate::contexts::daemon::domain::cache::{CacheEntry, PrOutput, RefreshMode};

impl From<RefreshModeDto> for RefreshMode {
    fn from(dto: RefreshModeDto) -> Self {
        match dto {
            RefreshModeDto::Hot => RefreshMode::Hot,
            RefreshModeDto::Warm => RefreshMode::Warm,
            RefreshModeDto::Terminal => RefreshMode::Terminal,
        }
    }
}

pub(super) fn entry_to_dtos(entry: &CacheEntry) -> Vec<EntryDto> {
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

pub(super) fn pr_outputs_from_dtos(pr_outputs_dto: &[PrOutputDto]) -> Vec<PrOutput> {
    pr_outputs_dto
        .iter()
        .map(|p| PrOutput {
            pr_id: p.pr_id,
            output: p.output.clone(),
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

fn cached_at_secs(entry: &CacheEntry) -> u64 {
    entry
        .fetched_at_wall()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn entry_to_dtos_expands_pr_outputs() {
        let mut entry = CacheEntry::new(PathBuf::from("/repo"), "feat/multi".to_owned(), 5);
        entry.update(
            "✓ Ready for merge #200 ✎ Ready for review #201".to_owned(),
            vec![
                PrOutput {
                    pr_id: 200,
                    output: "✓ Ready for merge #200".to_owned(),
                },
                PrOutput {
                    pr_id: 201,
                    output: "✎ Ready for review #201".to_owned(),
                },
            ],
            RefreshMode::Warm,
        );

        let dtos = entry_to_dtos(&entry);

        assert_eq!(dtos.len(), 2);
        assert_eq!(dtos[0].cwd, "/repo");
        assert_eq!(dtos[0].branch, "feat/multi");
        assert_eq!(dtos[0].pr_id, Some(200));
        assert_eq!(dtos[0].output, "✓ Ready for merge #200");
        assert_eq!(dtos[1].cwd, "/repo");
        assert_eq!(dtos[1].branch, "feat/multi");
        assert_eq!(dtos[1].pr_id, Some(201));
        assert_eq!(dtos[1].output, "✎ Ready for review #201");
    }

    #[test]
    fn entry_to_dtos_keeps_aggregate_output_without_pr_outputs() {
        let mut entry = CacheEntry::new(PathBuf::from("/repo"), "chore/no-pr".to_owned(), 5);
        entry.update("+ Create PR".to_owned(), vec![], RefreshMode::Warm);

        let dtos = entry_to_dtos(&entry);

        assert_eq!(dtos.len(), 1);
        assert_eq!(dtos[0].cwd, "/repo");
        assert_eq!(dtos[0].branch, "chore/no-pr");
        assert_eq!(dtos[0].pr_id, None);
        assert_eq!(dtos[0].output, "+ Create PR");
    }
}
