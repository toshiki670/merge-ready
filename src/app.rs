use std::process::ExitCode;

use clap::CommandFactory;

use crate::adapters::ConfigAdapter;
use crate::cli::{Cli, Command};
use crate::contexts::daemon::application::cache as daemon_cache_app;
use crate::contexts::daemon::infrastructure::daemon_client::{self as daemon_client, DaemonClient};
use crate::contexts::prompt::infrastructure::{gh::GhClient, logger::Logger};
use crate::contexts::prompt::interface::presentation::Presenter;
use crate::contexts::prompt::interface::{
    cli::prompt::{self},
    presentation::PresentationConfigPort,
};

pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Some(Command::Prompt(_args)) => {
            prompt::cached::run(
                crate::contexts::prompt::infrastructure::repo_id::get,
                daemon_client::query_via_daemon,
            );
            ExitCode::SUCCESS
        }
        Some(Command::Config(args)) => {
            let config_path = crate::contexts::config::infrastructure::toml_loader::config_path();
            crate::contexts::config::interface::cli::run(&args, config_path.as_deref())
        }
        Some(Command::Daemon(args)) => {
            crate::contexts::prompt::infrastructure::logger::init();
            let lifecycle =
                crate::contexts::daemon::infrastructure::daemon_lifecycle::DaemonLifecycle::new(
                    |repo_id: &str, cwd: &std::path::Path| {
                        let repo_id = repo_id.to_owned();
                        let (tokens, error) =
                            crate::contexts::prompt::application::prompt::fetch_output(
                                &GhClient::new_in(cwd.to_path_buf()),
                                &Logger,
                            );
                        let config = ConfigAdapter::load();
                        let output = if let Some(err) = error {
                            config.render_error_token(err)
                        } else {
                            Presenter::new(config).render_to_string(&tokens)
                        };
                        daemon_cache_app::update(&DaemonClient, &repo_id, &output);
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
