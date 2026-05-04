use super::port::{EntryView, WatchPort};

/// キャッシュエントリ一覧を取得するユースケース。
pub fn entries(port: &impl WatchPort) -> Option<Vec<EntryView>> {
    port.entries()
}
