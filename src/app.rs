use std::process::ExitCode;

use clap::CommandFactory;

use crate::cli::{Cli, Command};
use crate::contexts::configuration::application::config_service::ConfigService;
use crate::contexts::configuration::infrastructure::toml_loader::TomlConfigRepository;
use crate::contexts::merge_readiness::application::{
    OutputToken,
    errors::ErrorToken,
    prompt::{ExecutionMode, RepoIdPort},
};
use crate::contexts::merge_readiness::infrastructure::{gh::GhClient, logger::Logger};
use crate::contexts::merge_readiness::interface::{
    cli::prompt::{self},
    presentation::{PresentationConfigPort, Presenter},
};
use crate::contexts::status_cache::application::cache as status_cache_app;
use crate::contexts::status_cache::infrastructure::daemon_client::{
    self as daemon_client, DaemonClient,
};

struct InfraRepoIdPort;

impl RepoIdPort for InfraRepoIdPort {
    fn get(&self) -> Option<String> {
        crate::contexts::merge_readiness::infrastructure::repo_id::get()
    }
}

impl crate::contexts::merge_readiness::application::errors::ErrorLogger for Logger {
    fn log(&self, msg: &str) {
        crate::contexts::merge_readiness::infrastructure::logger::append_error(msg);
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
    let repo_id_port = InfraRepoIdPort;
    match cli.command {
        Some(Command::Prompt(args)) => match prompt::resolve_mode(&args) {
            ExecutionMode::Direct => {
                crate::contexts::merge_readiness::interface::cli::prompt::direct::run(
                    &GhClient::new(),
                    &Logger,
                    ConfigAdapter::load(),
                );
                ExitCode::SUCCESS
            }
            ExecutionMode::Cached => {
                prompt::cached::run(&repo_id_port, daemon_client::query_via_daemon);
                ExitCode::SUCCESS
            }
        },
        Some(Command::Config(args)) => crate::contexts::configuration::interface::cli::run(&args),
        Some(Command::Daemon(args)) => {
            let lifecycle =
                crate::contexts::status_cache::infrastructure::daemon_lifecycle::DaemonLifecycle::new(
                    |repo_id: &str, cwd: &std::path::Path| {
                        let repo_id = repo_id.to_owned();
                        let tokens = crate::contexts::merge_readiness::application::prompt::fetch_output(
                            &GhClient::new_in(cwd.to_path_buf()),
                            &Logger,
                        );
                        if let Some(tokens) = tokens {
                            let output =
                                Presenter::new(ConfigAdapter::load()).render_to_string(&tokens);
                            status_cache_app::update(&DaemonClient, &repo_id, &output);
                        }
                    },
                );
            crate::contexts::status_cache::interface::cli::daemon::run(args.subcommand, &lifecycle);
            ExitCode::SUCCESS
        }
        None => {
            let _ = Cli::command().print_help();
            ExitCode::SUCCESS
        }
    }
}
