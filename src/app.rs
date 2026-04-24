use std::process::ExitCode;

use clap::CommandFactory;

use crate::cli::{Cli, Command};
use crate::contexts::config::application::config_service::ConfigService;
use crate::contexts::config::application::port::{LoadConfigPort, UpdateConfigPort};
use crate::contexts::config::domain::config::Config;
use crate::contexts::config::infrastructure::toml_loader::TomlConfigRepository;
use crate::contexts::daemon::application::cache as daemon_cache_app;
use crate::contexts::daemon::infrastructure::daemon_client::{self as daemon_client, DaemonClient};
use crate::contexts::prompt::application::port::PromptStatusPort;
use crate::contexts::prompt::application::{OutputToken, errors::ErrorToken};
use crate::contexts::prompt::domain::branch_sync::BranchSync;
use crate::contexts::prompt::domain::ci_checks::CiChecks;
use crate::contexts::prompt::domain::error::RepositoryError;
use crate::contexts::prompt::domain::merge_ready::MergeReadiness;
use crate::contexts::prompt::domain::pr_state::PrLifecycle;
use crate::contexts::prompt::domain::review::Review;
use crate::contexts::prompt::infrastructure::{gh::GhClient, logger::Logger};
use crate::contexts::prompt::interface::presentation::Presenter;
use crate::contexts::prompt::interface::{
    cli::prompt::{self},
    presentation::PresentationConfigPort,
};

/// Bin-layer adapter that satisfies the config application ports by delegating
/// to the TOML-backed infrastructure implementation.
pub(crate) struct TomlConfigPortAdapter(TomlConfigRepository);

impl TomlConfigPortAdapter {
    pub(crate) const fn new() -> Self {
        Self(TomlConfigRepository)
    }
}

impl LoadConfigPort for TomlConfigPortAdapter {
    fn load(&self) -> Config {
        self.0.load()
    }
}

impl UpdateConfigPort for TomlConfigPortAdapter {
    fn load(&self) -> Config {
        self.0.load()
    }

    fn save(&self, config: &Config) -> Result<(), std::io::Error> {
        self.0.save(config)
    }
}

/// Bin-layer adapter that satisfies `PromptStatusPort` by delegating to the
/// gh-backed infrastructure implementation.
pub(crate) struct GhPromptAdapter(GhClient);

impl GhPromptAdapter {
    pub(crate) fn new_in(cwd: std::path::PathBuf) -> Self {
        Self(GhClient::new_in(cwd))
    }
}

impl PromptStatusPort for GhPromptAdapter {
    fn fetch_lifecycle(&self) -> Result<PrLifecycle, RepositoryError> {
        self.0.fetch_lifecycle()
    }

    fn fetch_sync_status(&self) -> Result<BranchSync, RepositoryError> {
        self.0.fetch_sync_status()
    }

    fn fetch_checks(&self) -> Result<CiChecks, RepositoryError> {
        self.0.fetch_checks()
    }

    fn fetch_review(&self) -> Result<Review, RepositoryError> {
        self.0.fetch_review()
    }

    fn fetch_readiness(&self) -> Result<MergeReadiness, RepositoryError> {
        self.0.fetch_readiness()
    }
}

pub(crate) struct ConfigAdapter(ConfigService);

impl ConfigAdapter {
    pub(crate) fn load() -> Self {
        Self(ConfigService::new(&TomlConfigPortAdapter::new()))
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
        Some(Command::Prompt(_args)) => {
            prompt::cached::run(
                crate::contexts::prompt::infrastructure::repo_id::get,
                daemon_client::query_via_daemon,
            );
            ExitCode::SUCCESS
        }
        Some(Command::Config(args)) => {
            let config_path = crate::contexts::config::infrastructure::toml_loader::config_path();
            crate::contexts::config::interface::cli::run(
                &args,
                &TomlConfigPortAdapter::new(),
                config_path.as_deref(),
            )
        }
        Some(Command::Daemon(args)) => {
            crate::contexts::prompt::infrastructure::logger::init();
            let lifecycle =
                crate::contexts::daemon::infrastructure::daemon_lifecycle::DaemonLifecycle::new(
                    |repo_id: &str, cwd: &std::path::Path| {
                        let repo_id = repo_id.to_owned();
                        let (tokens, error) =
                            crate::contexts::prompt::application::prompt::fetch_output(
                                &GhPromptAdapter::new_in(cwd.to_path_buf()),
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
