use std::process::ExitCode;

use clap::CommandFactory;

use crate::cli::{Cli, Command};
use crate::contexts::daemon::application::cache as daemon_cache_app;
use crate::contexts::daemon::domain::cache::RefreshMode;
use crate::contexts::daemon::infrastructure::daemon_client::DaemonClient;
use crate::contexts::evaluation::infrastructure::toml_loader::TomlConfigRepository;
use crate::contexts::evaluation::infrastructure::{gh::GhClient, logger::Logger};
use crate::contexts::evaluation::interface::prompt::CacheHint;

fn cache_hint_to_refresh_mode(hint: CacheHint) -> RefreshMode {
    match hint {
        CacheHint::Hot => RefreshMode::Hot,
        CacheHint::Warm => RefreshMode::Warm,
        CacheHint::Terminal => RefreshMode::Terminal,
    }
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Some(Command::Config) => {
            let config_path =
                crate::contexts::evaluation::infrastructure::toml_loader::config_path();
            crate::contexts::evaluation::interface::cli::config::run(config_path.as_deref())
        }
        Some(Command::Daemon(args)) => {
            crate::contexts::evaluation::infrastructure::logger::init();
            let lifecycle =
                crate::contexts::daemon::infrastructure::daemon_lifecycle::DaemonLifecycle::new(
                    // repo_id はブランチ変化を考慮して daemon_server が再導出して渡す
                    |repo_id: &str, cwd: &std::path::Path| {
                        let repo_id = repo_id.to_owned();
                        let client = GhClient::new_in(cwd.to_path_buf(), Logger);
                        let (output, hint) = crate::contexts::evaluation::interface::prompt::render(
                            &client,
                            &TomlConfigRepository,
                            &Logger,
                        );
                        let refresh_mode = cache_hint_to_refresh_mode(hint);
                        daemon_cache_app::update(&DaemonClient, &repo_id, &output, refresh_mode);
                    },
                );
            crate::contexts::daemon::interface::cli::daemon::run(args.subcommand, &lifecycle)
        }
        None => {
            let _ = Cli::command().print_help();
            ExitCode::SUCCESS
        }
    }
}
