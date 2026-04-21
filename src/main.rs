mod contexts;

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
    presentation::{PresentationConfigPort, Presenter},
};
use contexts::status_cache::application::cache as status_cache_app;
use contexts::status_cache::infrastructure::daemon_client::{self as daemon_client, DaemonClient};
use contexts::status_cache::interface::cli::DaemonCommand;

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
    // TODO: ConfigCommand も configuration::interface::cli に移動し、
    //       ここでは ConfigCommand を re-export して使う形に統一する。
    //       現状は main.rs に ConfigCommand 定義と CLI ロジックが漏洩している。
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

struct InfraRepoIdPort;

impl RepoIdPort for InfraRepoIdPort {
    fn get(&self) -> Option<String> {
        contexts::merge_readiness::infrastructure::repo_id::get()
    }
}

impl contexts::merge_readiness::application::errors::ErrorLogger for Logger {
    fn log(&self, msg: &str) {
        contexts::merge_readiness::infrastructure::logger::append_error(msg);
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
        Some(Command::Prompt(args)) => match prompt::resolve_mode(&args) {
            ExecutionMode::Direct => {
                contexts::merge_readiness::interface::cli::prompt::direct::run(
                    &GhClient::new(),
                    &Logger,
                    ConfigAdapter::load(),
                );
            }
            ExecutionMode::Cached => {
                prompt::cached::run(&repo_id_port, daemon_client::query_via_daemon);
            }
        },
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
        Some(Command::Daemon { subcommand }) => {
            let lifecycle =
                contexts::status_cache::infrastructure::daemon_lifecycle::DaemonLifecycle::new(
                    |repo_id: &str, cwd: &std::path::Path| {
                        let repo_id = repo_id.to_owned();
                        let tokens = contexts::merge_readiness::application::prompt::fetch_output(
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
            contexts::status_cache::interface::cli::daemon::run(subcommand, &lifecycle);
        }
        None => {
            let _ = Cli::command().print_help();
        }
    }
}
