use std::process::ExitCode;

use clap::CommandFactory;

use crate::cli::{Cli, Command};
use crate::contexts::daemon::application::cache as daemon_cache_app;
use crate::contexts::daemon::infrastructure::daemon_client::DaemonClient;
use crate::contexts::evaluation::application::config_service::ConfigService;
use crate::contexts::evaluation::infrastructure::toml_loader::TomlConfigRepository;
use crate::contexts::evaluation::infrastructure::{gh::GhClient, logger::Logger};
use crate::contexts::evaluation::interface::presentation::Presenter;

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
                    |repo_id: &str, cwd: &std::path::Path| {
                        let repo_id = repo_id.to_owned();
                        let client = GhClient::new_in(cwd.to_path_buf());
                        let (tokens, error, is_terminal) =
                            crate::contexts::evaluation::application::prompt::fetch_output(
                                &client, &Logger,
                            );
                        let output = Presenter::new(ConfigService::new(&TomlConfigRepository))
                            .render_output(&tokens, error);
                        daemon_cache_app::update(&DaemonClient, &repo_id, &output, is_terminal);
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
