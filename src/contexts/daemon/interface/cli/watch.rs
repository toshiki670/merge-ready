use std::fmt::Write as _;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};

use crate::contexts::daemon::application::port::{EntryView, WatchPort};
use crate::contexts::daemon::application::watch;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
// POLL_INTERVAL を細切れに sleep し、間で shutdown フラグをポーリングする。
// SIGINT 受信から終了までの遅延上限 = SHUTDOWN_TICK。
const SHUTDOWN_TICK: Duration = Duration::from_millis(50);

pub fn run(port: &impl WatchPort) -> ExitCode {
    let shutdown = Arc::new(AtomicBool::new(false));
    register_shutdown_signals(&shutdown);

    while !shutdown.load(Ordering::SeqCst) {
        clear_screen();
        if !draw(port) {
            return ExitCode::FAILURE;
        }
        if !sleep_until(POLL_INTERVAL, &shutdown) {
            break;
        }
    }
    ExitCode::SUCCESS
}

fn register_shutdown_signals(shutdown: &Arc<AtomicBool>) {
    for sig in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        let _ = signal_hook::flag::register(sig, Arc::clone(shutdown));
    }
}

/// 指定 duration を細切れに sleep する。途中で shutdown が立てば false を返す。
fn sleep_until(duration: Duration, shutdown: &AtomicBool) -> bool {
    let deadline = std::time::Instant::now() + duration;
    while std::time::Instant::now() < deadline {
        if shutdown.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(SHUTDOWN_TICK);
    }
    true
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

    let mut sorted: Vec<&EntryView> = entries.iter().collect();
    sorted.sort_by(|a, b| {
        a.cwd
            .cmp(&b.cwd)
            .then_with(|| a.branch.cmp(&b.branch))
            .then_with(|| a.pr_id.cmp(&b.pr_id))
    });

    let header = TableRow {
        cwd: "CWD".to_owned(),
        branch: "BRANCH".to_owned(),
        pr: "PR".to_owned(),
        status: "STATUS".to_owned(),
        cached_at: "CACHED AT".to_owned(),
    };
    let rows: Vec<TableRow> = sorted.iter().map(|e| TableRow::from_entry(e)).collect();
    let widths = TableWidths::from_rows(&header, &rows);

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<cwd_width$}  {:<branch_width$}  {:<pr_width$}  {:<status_width$}  {:>cached_at_width$}",
        header.cwd,
        header.branch,
        header.pr,
        header.status,
        header.cached_at,
        cwd_width = widths.cwd,
        branch_width = widths.branch,
        pr_width = widths.pr,
        status_width = widths.status,
        cached_at_width = widths.cached_at,
    );
    for row in &rows {
        // Rust の幅指定子は `chars()` 数ベース。ANSI 込みの status を
        // 見た目幅 `widths.status` で揃えるため、不可視 char 分を足し戻す。
        let status_pad = widths.status - visible_len(&row.status) + row.status.chars().count();
        let _ = writeln!(
            out,
            "{:<cwd_width$}  {:<branch_width$}  {:<pr_width$}  {:<status_pad$}  {:>cached_at_width$}",
            row.cwd,
            row.branch,
            row.pr,
            row.status,
            row.cached_at,
            cwd_width = widths.cwd,
            branch_width = widths.branch,
            pr_width = widths.pr,
            cached_at_width = widths.cached_at,
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
    cached_at: usize,
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
            cached_at: max_width(
                rows.iter().map(|row| row.cached_at.len()),
                header.cached_at.len(),
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
    format_elapsed(now.saturating_sub(cached_at_secs))
}

fn format_elapsed(elapsed_secs: u64) -> String {
    if elapsed_secs < 60 {
        format!("{elapsed_secs}s ago")
    } else if elapsed_secs < 3600 {
        format!("{}m ago", elapsed_secs / 60)
    } else {
        format!("{}h ago", elapsed_secs / 3600)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

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
    fn format_table_entries_sorted_by_cwd_then_branch_then_pr() {
        let rows = vec![
            make_view("/z/repo", "main", None, "✓ Ready", 5),
            make_view("/a/repo", "feat/2", Some(2), "✓ Ready", 5),
            make_view("/a/repo", "feat/1", Some(1), "✓ Ready", 5),
            make_view("/a/repo", "feat/2", Some(1), "✓ Ready", 5),
        ];
        let table = format_table(&rows);
        let lines: Vec<&str> = table.lines().skip(1).collect();
        assert!(
            lines[0].contains("/a/repo") && lines[0].contains("feat/1"),
            "1行目: /a/repo feat/1"
        );
        assert!(
            lines[1].contains("/a/repo") && lines[1].contains("feat/2") && lines[1].contains("#1"),
            "2行目: /a/repo feat/2 #1"
        );
        assert!(
            lines[2].contains("/a/repo") && lines[2].contains("feat/2") && lines[2].contains("#2"),
            "3行目: /a/repo feat/2 #2"
        );
        assert!(lines[3].contains("/z/repo"), "4行目: /z/repo");
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

    #[rstest]
    #[case(5, "5s ago")]
    #[case(59, "59s ago")]
    #[case(60, "1m ago")]
    #[case(90, "1m ago")]
    #[case(3600, "1h ago")]
    #[case(7200, "2h ago")]
    fn format_elapsed_various_durations(#[case] elapsed_secs: u64, #[case] expected: &str) {
        assert_eq!(format_elapsed(elapsed_secs), expected);
    }

    #[test]
    fn format_table_right_aligns_cached_at_column() {
        // status は ASCII（visible_len == len）にして他列の幅ぶれを排除し、
        // CACHED AT 列の右揃えのみを検証する。
        let rows = vec![
            make_view("/repo", "main", Some(1), "Ready", 5),
            make_view("/repo", "main", Some(2), "Ready", 7200),
        ];
        let table = format_table(&rows);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 3, "header + 2 rows: {table:?}");

        let widths: Vec<usize> = lines.iter().map(|l| visible_len(l)).collect();
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "全行の表示幅が揃うべき: {widths:?} / {table:?}",
        );

        // "CACHED AT" (9 文字) がヘッダーで最長。短い値 "5s ago" (6 文字) は
        // 3 文字の左パディングが付与された形で末尾に現れる。
        assert!(
            lines[0].ends_with("CACHED AT"),
            "ヘッダーも右端で揃うべき: {:?}",
            lines[0],
        );
        assert!(
            lines[1].ends_with("   5s ago"),
            "短い相対時刻は右揃えされるべき: {:?}",
            lines[1],
        );
        assert!(
            lines[2].ends_with("   2h ago"),
            "短い相対時刻は右揃えされるべき: {:?}",
            lines[2],
        );
    }
}
