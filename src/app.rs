use std::process::ExitCode;

use clap::CommandFactory;

use crate::cli::{Cli, Command};
use crate::contexts::config::application::config_service::ConfigService;
use crate::contexts::config::infrastructure::toml_loader::TomlConfigRepository;
use crate::contexts::daemon::application::cache as daemon_cache_app;
use crate::contexts::daemon::infrastructure::daemon_client::{self as daemon_client, DaemonClient};
use crate::contexts::prompt::application::{
    OutputToken, errors::ErrorToken, prompt::ExecutionMode,
};
use crate::contexts::prompt::infrastructure::{gh::GhClient, logger::Logger};
use crate::contexts::prompt::interface::presentation::Presenter;
use crate::contexts::prompt::interface::{
    cli::prompt::{self},
    presentation::PresentationConfigPort,
};

impl crate::contexts::prompt::application::errors::ErrorLogger for Logger {
    fn log(&self, msg: &str) {
        crate::contexts::prompt::infrastructure::logger::append_error(msg);
    }
}

pub(crate) struct ConfigAdapter(ConfigService);

impl ConfigAdapter {
    pub(crate) fn load() -> Self {
        Self(ConfigService::new(&TomlConfigRepository))
    }
}

impl PresentationConfigPort for ConfigAdapter {
    fn render_token(&self, token: &OutputToken) -> String {
        match token {
            OutputToken::MergeReady => self.0.render_merge_ready(),
            OutputToken::Conflict => self.0.render_conflict(),
            OutputToken::UpdateBranch => self.0.render_update_branch(),
            OutputToken::SyncUnknown => self.0.render_sync_unknown(),
            OutputToken::CiFail => self.0.render_ci_fail(),
            OutputToken::CiAction => self.0.render_ci_action(),
            OutputToken::ReviewRequested => self.0.render_review(),
        }
    }

    fn render_error_token(&self, token: ErrorToken) -> String {
        match token {
            ErrorToken::AuthRequired => self.0.render_auth_required(),
            ErrorToken::RateLimited => self.0.render_rate_limited(),
            ErrorToken::ApiError => self.0.render_api_error(),
        }
    }
}

pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Some(Command::Prompt(args)) => match prompt::resolve_mode(&args) {
            ExecutionMode::Direct => {
                crate::contexts::prompt::interface::cli::prompt::direct::run(
                    &GhClient::new(),
                    &Logger,
                    ConfigAdapter::load(),
                );
                ExitCode::SUCCESS
            }
            ExecutionMode::Cached => {
                prompt::cached::run(
                    crate::contexts::prompt::infrastructure::repo_id::get,
                    daemon_client::query_via_daemon,
                );
                ExitCode::SUCCESS
            }
        },
        Some(Command::Config(args)) => {
            let config_path = crate::contexts::config::infrastructure::toml_loader::config_path();
            crate::contexts::config::interface::cli::run(
                &args,
                &TomlConfigRepository,
                config_path.as_deref(),
            )
        }
        Some(Command::Daemon(args)) => {
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
            crate::contexts::daemon::interface::cli::daemon::run(args.subcommand, &lifecycle);
            ExitCode::SUCCESS
        }
        None => {
            let _ = Cli::command().print_help();
            ExitCode::SUCCESS
        }
    }
}
