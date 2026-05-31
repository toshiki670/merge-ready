use std::path::PathBuf;

/// ソケット・PID ファイル名に含めるバージョン。
///
/// バージョンごとに別ファイルを使うことで、複数バージョンのデーモンが共存しても
/// 衝突せず、新デーモンが古いデーモンを安全に検知・停止できる。
///
/// なお #380 で temp ディレクトリ名を `merge-ready` → `merge-ready-{uid}` に変えたため、
/// 旧バージョン検知（`old_daemon_pid_files`）は自分の `base_dir` に加えて pre-#380 の旧命名
/// 兄弟ディレクトリ `merge-ready` も走査する。将来ディレクトリ命名を変える際は、上記の
/// 「新デーモンが旧デーモンを検知・停止する」不変条件を保つよう走査集合と E2E を更新すること。
const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

const SOCKET_PREFIX: &str = "daemon-";
const SOCKET_EXT: &str = ".sock";
const PID_EXT: &str = ".pid";

/// #380 以前の temp ディレクトリ名（uid 名前空間分離の前）。
const LEGACY_DIR_NAME: &str = "merge-ready";

#[derive(Clone)]
pub struct Paths {
    base_dir: PathBuf,
}

impl Paths {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn socket_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{SOCKET_PREFIX}{DAEMON_VERSION}{SOCKET_EXT}"))
    }

    pub fn pid_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{SOCKET_PREFIX}{DAEMON_VERSION}{PID_EXT}"))
    }

    pub fn lock_path(&self) -> PathBuf {
        self.base_dir.join("daemon.lock")
    }

    /// 旧バージョンの PID ファイルを列挙する（現バージョンは除外）。
    ///
    /// 列挙対象は `daemon-{version}.pid` 形式のファイルのみ。新デーモン起動時に
    /// 旧バージョンの停止・stale ファイル削除のために使用する。
    ///
    /// 自分の `base_dir` に加えて、pre-#380 の旧命名兄弟ディレクトリ `merge-ready`（あれば）も
    /// 走査する。これにより `merge-ready` → `merge-ready-{uid}` のバージョン跨ぎでも、
    /// 旧ディレクトリに残った旧デーモンを検知・停止できる。
    pub fn old_daemon_pid_files(&self) -> Vec<PathBuf> {
        let current_pid_name = format!("{SOCKET_PREFIX}{DAEMON_VERSION}{PID_EXT}");
        let mut dirs = vec![self.base_dir.clone()];
        dirs.extend(self.legacy_sibling_dir());
        dirs.into_iter()
            .filter_map(|dir| std::fs::read_dir(dir).ok())
            .flat_map(|entries| entries.filter_map(Result::ok).map(|e| e.path()))
            .filter(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    n.starts_with(SOCKET_PREFIX) && n.ends_with(PID_EXT) && n != current_pid_name
                })
            })
            .collect()
    }

    /// pre-#380 の旧命名兄弟ディレクトリ `merge-ready` を返す（該当する場合のみ）。
    ///
    /// `base_dir` が `merge-ready-{uid}`（suffix が空でなく全て数字）のときだけ、その親の下の
    /// `merge-ready` を返す。これにより uid 名前空間ディレクトリだけが pre-#380 の対応物を
    /// 走査対象とし、ランダム名のテスト用ディレクトリや本物の `merge-ready` 自身は誤走査しない。
    fn legacy_sibling_dir(&self) -> Option<PathBuf> {
        let name = self.base_dir.file_name()?.to_str()?;
        let suffix = name.strip_prefix(&format!("{LEGACY_DIR_NAME}-"))?;
        if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let legacy = self.base_dir.parent()?.join(LEGACY_DIR_NAME);
        (legacy != self.base_dir).then_some(legacy)
    }
}

impl Default for Paths {
    fn default() -> Self {
        let base = std::env::var("MERGE_READY_BASE_DIR").map_or_else(|_| base_dir(), PathBuf::from);
        Self::new(base)
    }
}

pub fn base_dir() -> PathBuf {
    std::env::temp_dir().join(dir_name())
}

/// 起動ユーザの uid で temp 配下のディレクトリ名を名前空間分離する。
///
/// uid 別ディレクトリにすることで、`TMPDIR` が共有書き込み可能な環境
/// （例: `TMPDIR=/tmp`）でも、他ユーザが lock/pid/socket を置くディレクトリを
/// 先回り作成する攻撃を防ぐ（defense-in-depth）。unix では実 uid を使い、
/// 非 unix（uid の概念がない）では固定名にフォールバックする。
fn dir_name() -> String {
    std::cfg_select! {
        unix => dir_name_for(Some(rustix::process::getuid().as_raw())),
        _ => dir_name_for(None),
    }
}

/// uid から temp 配下のディレクトリ名を決める純粋関数。
///
/// uid がある場合は `merge-ready-{uid}` で名前空間を分離し、無い場合
/// （非 unix）は固定の `merge-ready` を使う。
fn dir_name_for(uid: Option<u32>) -> String {
    match uid {
        Some(uid) => format!("merge-ready-{uid}"),
        None => "merge-ready".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Paths, dir_name_for};
    use std::path::PathBuf;

    #[test]
    fn dir_name_includes_uid_when_available() {
        assert_eq!(dir_name_for(Some(1000)), "merge-ready-1000");
        assert_eq!(dir_name_for(Some(0)), "merge-ready-0");
    }

    #[test]
    fn dir_name_falls_back_to_plain_when_uid_unavailable() {
        assert_eq!(dir_name_for(None), "merge-ready");
    }

    #[test]
    fn paths_from_custom_dir() {
        let p = Paths::new("/tmp/test-merge-ready".into());
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(
            p.socket_path(),
            std::path::PathBuf::from(format!("/tmp/test-merge-ready/daemon-{version}.sock"))
        );
        assert_eq!(
            p.pid_path(),
            std::path::PathBuf::from(format!("/tmp/test-merge-ready/daemon-{version}.pid"))
        );
        assert_eq!(
            p.lock_path(),
            std::path::PathBuf::from("/tmp/test-merge-ready/daemon.lock")
        );
    }

    #[test]
    fn old_daemon_pid_files_excludes_current_version() {
        let dir = tempfile::tempdir().unwrap();
        let version = env!("CARGO_PKG_VERSION");

        // 現バージョン + 旧バージョン 2 つ + 無関係なファイル
        std::fs::write(dir.path().join(format!("daemon-{version}.pid")), "1").unwrap();
        std::fs::write(dir.path().join("daemon-0.0.0.pid"), "2").unwrap();
        std::fs::write(dir.path().join("daemon-0.1.0.pid"), "3").unwrap();
        std::fs::write(dir.path().join("daemon.lock"), "").unwrap();
        std::fs::write(dir.path().join("other.pid"), "").unwrap();

        let paths = Paths::new(dir.path().to_path_buf());
        let mut found: Vec<String> = paths
            .old_daemon_pid_files()
            .into_iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        found.sort();
        assert_eq!(found, vec!["daemon-0.0.0.pid", "daemon-0.1.0.pid"]);
    }

    #[test]
    fn old_daemon_pid_files_scans_legacy_sibling_dir() {
        // #390: base_dir が `merge-ready-{uid}` のとき、pre-#380 の旧命名兄弟
        // ディレクトリ `merge-ready` も走査する。
        let parent = tempfile::tempdir().unwrap();
        let base = parent.path().join("merge-ready-0");
        let legacy = parent.path().join("merge-ready");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(base.join("daemon-0.0.0.pid"), "1").unwrap();
        std::fs::write(legacy.join("daemon-0.1.0.pid"), "2").unwrap();

        let paths = Paths::new(base);
        let mut found: Vec<PathBuf> = paths.old_daemon_pid_files();
        found.sort();
        assert_eq!(
            found,
            vec![
                legacy.join("daemon-0.1.0.pid"),
                parent.path().join("merge-ready-0").join("daemon-0.0.0.pid"),
            ]
        );
    }

    #[test]
    fn old_daemon_pid_files_ignores_sibling_when_suffix_not_uid() {
        // base_dir の suffix が uid（数字）でない場合は旧命名兄弟ディレクトリを走査しない。
        // テスト用の隔離ディレクトリや本物の `$TMPDIR/merge-ready` を誤って掃除しないため。
        let parent = tempfile::tempdir().unwrap();
        let base = parent.path().join("merge-ready-e2e");
        let legacy = parent.path().join("merge-ready");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(base.join("daemon-0.0.0.pid"), "1").unwrap();
        std::fs::write(legacy.join("daemon-0.1.0.pid"), "2").unwrap();

        let paths = Paths::new(base.clone());
        let found: Vec<PathBuf> = paths.old_daemon_pid_files();
        assert_eq!(found, vec![base.join("daemon-0.0.0.pid")]);
    }
}
