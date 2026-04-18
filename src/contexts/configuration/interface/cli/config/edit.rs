use std::path::Path;

pub fn run(path: &Path) {
    use std::ffi::OsString;
    let editor = std::env::var_os("VISUAL")
        .or_else(|| std::env::var_os("EDITOR"))
        .unwrap_or_else(|| OsString::from("vi"));

    ensure_config_file(path);
    let _ = std::process::Command::new(editor).arg(path).status();
}

fn ensure_config_file(path: &Path) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, default_config_toml());
}

fn default_config_toml() -> &'static str {
    "\
# merge-ready configuration
# All fields are optional; omit a section to use built-in defaults.

# [merge_ready]
# symbol = \"✓\"
# label = \"merge-ready\"
# format = \"$symbol $label\"

# [conflict]
# symbol = \"✗\"
# label = \"conflict\"

# [update_branch]
# symbol = \"✗\"
# label = \"update-branch\"

# [sync_unknown]
# symbol = \"?\"
# label = \"sync-unknown\"

# [ci_fail]
# symbol = \"✗\"
# label = \"ci-fail\"

# [ci_action]
# symbol = \"⚠\"
# label = \"ci-action\"

# [review]
# symbol = \"⚠\"
# label = \"review\"
"
}
