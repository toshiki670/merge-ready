use std::path::Path;

/// # Errors
///
/// 設定ファイルの作成、エディタの起動、またはエディタが非ゼロで終了した場合に `Err` を返す。
pub fn run(path: &Path) -> Result<(), std::io::Error> {
    use std::ffi::OsString;
    let editor = std::env::var_os("VISUAL")
        .or_else(|| std::env::var_os("EDITOR"))
        .unwrap_or_else(|| OsString::from("vi"));

    ensure_config_file(path)?;
    let status = std::process::Command::new(&editor)
        .arg(path)
        .status()
        .map_err(|e| {
            std::io::Error::other(format!("failed to launch editor {}: {e}", editor.display()))
        })?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "editor {} exited with {status}",
            editor.display()
        )));
    }
    Ok(())
}

fn ensure_config_file(path: &Path) -> Result<(), std::io::Error> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let config = crate::contexts::evaluation::application::config_service::default_display_config();
    let content = toml::to_string_pretty(&config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, content.as_bytes())
}
