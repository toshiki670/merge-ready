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
    let content = crate::contexts::configuration::application::config_updater::default_config_toml();
    let _ = std::fs::write(path, content.as_bytes());
}
