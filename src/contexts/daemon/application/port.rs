/// キャッシュエントリの表示用ビューモデル。
pub struct EntryView {
    pub cwd: String,
    pub branch: String,
    pub output: String,
    pub cached_at_secs: u64,
}

/// `watch` ユースケースが必要とするアダプタポート。
pub trait WatchPort {
    fn entries(&self) -> Option<Vec<EntryView>>;
}
