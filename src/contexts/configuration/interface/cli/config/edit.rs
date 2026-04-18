use std::path::Path;

pub fn run(path: &Path) -> Result<(), std::io::Error> {
    use std::ffi::OsString;
    let editor = std::env::var_os("VISUAL")
        .or_else(|| std::env::var_os("EDITOR"))
        .unwrap_or_else(|| OsString::from("vi"));

    ensure_config_file(path)?;
    // TODO: エディタの終了コードを確認してエラー処理する
    let _ = std::process::Command::new(editor).arg(path).status();
    Ok(())
}

fn ensure_config_file(path: &Path) -> Result<(), std::io::Error> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let config = crate::contexts::configuration::application::config_updater::default_config();
    let content = toml::to_string_pretty(&config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, content.as_bytes())
}
