/// キャッシュ・ロックファイルを格納する一時ディレクトリのパスを返す。
///
/// - macOS: `$TMPDIR/merge-ready/`
///   `$TMPDIR` は OS が各ユーザーに割り当てる専用ディレクトリ（例: `/var/folders/.../T/`）。
///   [`std::env::temp_dir()`] が `$TMPDIR` を返すため、別途 UID を付加しなくてもユーザー間で衝突しない。
///
/// - Linux: `/tmp/merge-ready-{uid}/`
///   `/tmp` はすべてのユーザーが共有するため、`/proc/self` のメタデータから取得した UID を
///   ディレクトリ名に付加してユーザー間衝突を防ぐ。
///   `$TMPDIR` が設定されている場合は [`std::env::temp_dir()`] がその値を使用する。
pub(super) fn cache_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(dir_name())
}

const DIR_NAME: &str = "merge-ready";

fn dir_name() -> String {
    std::cfg_select! {
        target_os = "linux" => {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = std::fs::metadata("/proc/self") {
                format!("{DIR_NAME}-{}", meta.uid())
            } else {
                DIR_NAME.to_owned()
            }
        },
        _ => DIR_NAME.to_owned(),
    }
}
