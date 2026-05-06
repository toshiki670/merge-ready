use std::fmt::Write as _;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};

use crate::contexts::daemon::application::port::{EntryView, WatchPort};
use crate::contexts::daemon::application::watch;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

pub fn run(port: &impl WatchPort) -> ExitCode {
    loop {
        clear_screen();
        if !draw(port) {
            return ExitCode::FAILURE;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn clear_screen() {
    let mut stdout = std::io::stdout();
    let _ = execute!(
        stdout,
        terminal::Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    );
}

fn draw(port: &impl WatchPort) -> bool {
    match watch::entries(port) {
        None => {
            println!("daemon is not running");
            false
        }
        Some(entries) => {
            print!("{}", format_table(&entries));
            true
        }
    }
}

fn format_table(entries: &[EntryView]) -> String {
    if entries.is_empty() {
        return "no entries cached\n".to_owned();
    }

    let header = TableRow {
        cwd: "CWD".to_owned(),
        branch: "BRANCH".to_owned(),
        pr: "PR".to_owned(),
        status: "STATUS".to_owned(),
        cached_at: "CACHED AT".to_owned(),
    };
    let rows: Vec<TableRow> = entries.iter().map(TableRow::from_entry).collect();
    let widths = TableWidths::from_rows(&header, &rows);

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<cwd_width$}  {:<branch_width$}  {:<pr_width$}  {:<status_width$}  {}",
        header.cwd,
        header.branch,
        header.pr,
        header.status,
        header.cached_at,
        cwd_width = widths.cwd,
        branch_width = widths.branch,
        pr_width = widths.pr,
        status_width = widths.status,
    );
    for row in &rows {
        let status_pad = widths.status - visible_len(&row.status) + row.status.len();
        let _ = writeln!(
            out,
            "{:<cwd_width$}  {:<branch_width$}  {:<pr_width$}  {:<status_pad$}  {}",
            row.cwd,
            row.branch,
            row.pr,
            row.status,
            row.cached_at,
            cwd_width = widths.cwd,
            branch_width = widths.branch,
            pr_width = widths.pr,
        );
    }
    out
}

struct TableRow {
    cwd: String,
    branch: String,
    pr: String,
    status: String,
    cached_at: String,
}

impl TableRow {
    fn from_entry(entry: &EntryView) -> Self {
        Self {
            cwd: entry.cwd.clone(),
            branch: entry.branch.clone(),
            pr: format_pr_id(entry.pr_id),
            status: entry.output.clone(),
            cached_at: format_cached_at(entry.cached_at_secs),
        }
    }
}

struct TableWidths {
    cwd: usize,
    branch: usize,
    pr: usize,
    status: usize,
}

impl TableWidths {
    fn from_rows(header: &TableRow, rows: &[TableRow]) -> Self {
        Self {
            cwd: max_width(rows.iter().map(|row| row.cwd.len()), header.cwd.len()),
            branch: max_width(rows.iter().map(|row| row.branch.len()), header.branch.len()),
            pr: max_width(rows.iter().map(|row| row.pr.len()), header.pr.len()),
            status: max_width(
                rows.iter().map(|row| visible_len(&row.status)),
                header.status.len(),
            ),
        }
    }
}

fn max_width(widths: impl Iterator<Item = usize>, header_width: usize) -> usize {
    widths.max().unwrap_or(0).max(header_width)
}

fn format_pr_id(pr_id: Option<u64>) -> String {
    pr_id.map_or_else(String::new, |id| format!("#{id}"))
}

fn visible_len(s: &str) -> usize {
    // ANSI エスケープシーケンス（ESC [ ... m）を除いた表示幅
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            len += 1;
        }
    }
    len
}

fn format_cached_at(cached_at_secs: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let elapsed = now.saturating_sub(cached_at_secs);
    if elapsed < 60 {
        format!("{elapsed}s ago")
    } else if elapsed < 3600 {
        format!("{}m ago", elapsed / 60)
    } else {
        format!("{}h ago", elapsed / 3600)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view(
        cwd: &str,
        branch: &str,
        pr_id: Option<u64>,
        output: &str,
        age_secs: u64,
    ) -> EntryView {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        EntryView {
            cwd: cwd.to_owned(),
            branch: branch.to_owned(),
            pr_id,
            output: output.to_owned(),
            cached_at_secs: now.saturating_sub(age_secs),
        }
    }

    #[test]
    fn format_table_empty_returns_no_entries_message() {
        let table = format_table(&[]);
        assert!(table.contains("no entries cached"));
    }

    #[test]
    fn format_table_shows_header() {
        let view = make_view("/repo", "main", Some(123), "✓ Ready", 5);
        let table = format_table(&[view]);
        assert!(table.contains("CWD"));
        assert!(table.contains("BRANCH"));
        assert!(table.contains("PR"));
        assert!(table.contains("STATUS"));
        assert!(table.contains("CACHED AT"));
    }

    #[test]
    fn format_table_shows_all_columns() {
        let view = make_view(
            "/repo/myapp",
            "feat/123",
            Some(123),
            "✓ Ready for merge",
            10,
        );
        let table = format_table(&[view]);
        assert!(table.contains("/repo/myapp"));
        assert!(table.contains("feat/123"));
        assert!(table.contains("#123"));
        assert!(table.contains("✓ Ready for merge"));
        assert!(table.contains("ago"));
    }

    #[test]
    fn format_table_leaves_pr_column_empty_without_pr_id() {
        let view = make_view("/repo/myapp", "chore/deps", None, "+ Create PR", 10);
        let table = format_table(&[view]);

        assert!(table.contains("PR"));
        assert!(!table.contains('#'));
        assert!(table.contains("+ Create PR"));
    }

    #[test]
    fn format_table_shows_multiple_pr_rows() {
        let rows = vec![
            make_view(
                "/repo/myapp",
                "feat/multi",
                Some(200),
                "✓ Ready for merge #200",
                10,
            ),
            make_view(
                "/repo/myapp",
                "feat/multi",
                Some(201),
                "✎ Ready for review #201",
                10,
            ),
        ];

        let table = format_table(&rows);

        assert!(table.contains("#200"));
        assert!(table.contains("#201"));
        assert!(table.contains("✓ Ready for merge #200"));
        assert!(table.contains("✎ Ready for review #201"));
    }

    #[test]
    fn format_cached_at_seconds() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let s = format_cached_at(now.saturating_sub(5));
        assert!(s.ends_with("s ago"));
    }

    #[test]
    fn format_cached_at_minutes() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let s = format_cached_at(now.saturating_sub(90));
        assert!(s.ends_with("m ago"));
    }

    #[test]
    fn format_cached_at_hours() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let s = format_cached_at(now.saturating_sub(7200));
        assert!(s.ends_with("h ago"));
    }
}
