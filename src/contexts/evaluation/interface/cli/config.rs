use std::path::Path;
use std::process::ExitCode;

pub mod edit;

pub fn run(config_path: Option<&Path>) -> ExitCode {
    let Some(path) = config_path else {
        eprintln!(
            "failed to edit config: could not determine config path (HOME or XDG_CONFIG_HOME required)"
        );
        return ExitCode::FAILURE;
    };
    if let Err(e) = edit::run(path) {
        eprintln!("failed to edit config: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
