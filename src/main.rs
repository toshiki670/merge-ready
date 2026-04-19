mod cached;
mod contexts;
mod refresh;

use clap::{CommandFactory, Parser, Subcommand};
use contexts::configuration::application::config_service::ConfigService;
use contexts::configuration::infrastructure::toml_loader::TomlConfigRepository;
use contexts::merge_readiness::application::{
    OutputToken,
    errors::ErrorToken,
    prompt::{ExecutionMode, RepoIdPort},
};
use contexts::merge_readiness::infrastructure::{gh::GhClient, logger::Logger};
use contexts::merge_readiness::interface::{
    cli::prompt::{self, AFTER_HELP, PromptArgs},
    presentation::PresentationConfigPort,
};

#[derive(Parser)]
#[command(
    name = "merge-ready",
    about = "PR merge status for your shell prompt",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show PR merge status for your shell prompt
    #[command(after_help = AFTER_HELP)]
    Prompt(PromptArgs),
    /// Manage the configuration file
    Config {
        #[command(subcommand)]
        subcommand: ConfigCommand,
    },
    /// Manage the background cache daemon
    Daemon {
        #[command(subcommand)]
        subcommand: DaemonCommand,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Open the configuration file in an editor (creates it with defaults if absent)
    Edit,
    /// Update the configuration file to the latest schema (preserves valid keys, removes obsolete ones, adds missing ones with defaults)
    Update,
}

#[derive(Subcommand)]
enum DaemonCommand {
    /// Start the background cache daemon (blocks; use as a background process)
    Start,
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
}

struct InfraRepoIdPort;

impl RepoIdPort for InfraRepoIdPort {
    fn get(&self) -> Option<String> {
        contexts::merge_readiness::infrastructure::repo_id::get()
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

fn main() {
    let repo_id_port = InfraRepoIdPort;
    match Cli::parse().command {
        Some(Command::Prompt(args)) => {
            let Some(mode) = prompt::resolve_mode(&args, &repo_id_port) else {
                return;
            };
            match mode {
                ExecutionMode::Direct => {
                    contexts::merge_readiness::interface::cli::prompt::direct::run(
                        &GhClient::new(),
                        &Logger,
                        ConfigAdapter::load(),
                    );
                }
                ExecutionMode::Cached => cached::run(),
                ExecutionMode::BackgroundRefresh { repo_id } => refresh::run(&repo_id),
            }
        }
        Some(Command::Config { subcommand }) => match subcommand {
            ConfigCommand::Edit => {
                let Some(path) =
                    contexts::configuration::infrastructure::toml_loader::config_path()
                else {
                    eprintln!(
                        "failed to edit config: could not determine config path (HOME or XDG_CONFIG_HOME required)"
                    );
                    std::process::exit(1);
                };
                if let Err(e) = contexts::configuration::interface::cli::config::edit::run(&path) {
                    eprintln!("failed to edit config: {e}");
                    std::process::exit(1);
                }
            }
            ConfigCommand::Update => {
                if let Err(e) = contexts::configuration::interface::cli::config::update::run(
                    &TomlConfigRepository,
                ) {
                    eprintln!("failed to update config: {e}");
                    std::process::exit(1);
                }
            }
        },
        Some(Command::Daemon { subcommand }) => match subcommand {
            DaemonCommand::Start => {
                contexts::status_cache::interface::cli::daemon::start();
            }
            DaemonCommand::Stop => {
                contexts::status_cache::interface::cli::daemon::stop();
            }
            DaemonCommand::Status => {
                contexts::status_cache::interface::cli::daemon::status();
            }
        },
        None => {
            let _ = Cli::command().print_help();
        }
    }
}
