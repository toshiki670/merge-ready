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

    let header = ("CWD", "BRANCH", "STATUS", "CACHED AT");
    let rows: Vec<(String, String, String, String)> = entries
        .iter()
        .map(|e| {
            (
                e.cwd.clone(),
                e.branch.clone(),
                e.output.clone(),
                format_cached_at(e.cached_at_secs),
            )
        })
        .collect();

    let w0 = rows
        .iter()
        .map(|r| r.0.len())
        .max()
        .unwrap_or(0)
        .max(header.0.len());
    let w1 = rows
        .iter()
        .map(|r| r.1.len())
        .max()
        .unwrap_or(0)
        .max(header.1.len());
    let w2 = rows
        .iter()
        .map(|r| visible_len(&r.2))
        .max()
        .unwrap_or(0)
        .max(header.2.len());

    let mut out = String::new();
    out.push_str(&format!(
        "{:<w0$}  {:<w1$}  {:<w2$}  {}\n",
        header.0, header.1, header.2, header.3,
    ));
    for row in &rows {
        let pad = w2 - visible_len(&row.2) + row.2.len();
        out.push_str(&format!(
            "{:<w0$}  {:<w1$}  {:<pad$}  {}\n",
            row.0, row.1, row.2, row.3,
        ));
    }
    out
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

    fn make_view(cwd: &str, branch: &str, output: &str, age_secs: u64) -> EntryView {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        EntryView {
            cwd: cwd.to_owned(),
            branch: branch.to_owned(),
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
        let view = make_view("/repo", "main", "✓ Ready", 5);
        let table = format_table(&[view]);
        assert!(table.contains("CWD"));
        assert!(table.contains("BRANCH"));
        assert!(table.contains("STATUS"));
        assert!(table.contains("CACHED AT"));
    }

    #[test]
    fn format_table_shows_all_columns() {
        let view = make_view("/repo/myapp", "feat/123", "✓ Ready for merge", 10);
        let table = format_table(&[view]);
        assert!(table.contains("/repo/myapp"));
        assert!(table.contains("feat/123"));
        assert!(table.contains("✓ Ready for merge"));
        assert!(table.contains("ago"));
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
