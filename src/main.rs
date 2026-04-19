mod contexts;

use clap::{CommandFactory, Parser, Subcommand};
use contexts::configuration::application::config_service::ConfigService;
use contexts::configuration::infrastructure::toml_loader::TomlConfigRepository;
use contexts::merge_readiness::application::{
    OutputToken,
    cache::{CachePort, CacheState},
    errors::ErrorToken,
    prompt::{ExecutionMode, PromptEffect, RepoIdPort},
};
use contexts::merge_readiness::infrastructure::{gh::GhClient, logger::Logger};
use contexts::merge_readiness::interface::{
    cli::prompt::{self, AFTER_HELP, PromptArgs},
    presentation::{PresentationConfigPort, Presenter},
};
use contexts::status_cache::application::cache::{self as status_cache_app, CacheQueryResult};
use contexts::status_cache::infrastructure::daemon_client::DaemonClient;

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
    /// Fetch fresh data and notify the daemon cache [internal: spawned by daemon]
    #[command(hide = true)]
    Refresh {
        #[arg(long)]
        repo_id: String,
    },
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

struct DaemonCacheAdapter;

impl CachePort for DaemonCacheAdapter {
    fn check(&self, repo_id: &str) -> CacheState {
        match status_cache_app::query(&DaemonClient, repo_id) {
            CacheQueryResult::Fresh(s) => CacheState::Fresh(s),
            CacheQueryResult::Stale(s) => CacheState::Stale(s),
            // Miss: daemon が refresh 予約済み。Unavailable: lazy_start 済み → next call でヒット
            CacheQueryResult::Miss | CacheQueryResult::Unavailable => CacheState::Miss,
        }
    }
}

fn run_cached_prompt(repo_id_port: &impl RepoIdPort) {
    let cache = DaemonCacheAdapter;
    match contexts::merge_readiness::application::prompt::resolve_cached(repo_id_port, &cache) {
        PromptEffect::NoOutput => {}
        PromptEffect::Show(s) | PromptEffect::ShowAndRefresh(s) => print!("{s}"),
        PromptEffect::ShowLoadingAndRefresh => print!("? loading"),
    }
}

fn run_daemon_refresh(repo_id: &str) {
    let tokens =
        contexts::merge_readiness::application::prompt::fetch_output(&GhClient::new(), &Logger);
    if let Some(tokens) = tokens {
        let output = Presenter::new(ConfigAdapter::load()).render_to_string(&tokens);
        status_cache_app::update(&DaemonClient, repo_id, &output);
    }
}

fn main() {
    let repo_id_port = InfraRepoIdPort;
    match Cli::parse().command {
        Some(Command::Prompt(args)) => match prompt::resolve_mode(&args, &repo_id_port) {
            ExecutionMode::Direct => {
                contexts::merge_readiness::interface::cli::prompt::direct::run(
                    &GhClient::new(),
                    &Logger,
                    ConfigAdapter::load(),
                );
            }
            ExecutionMode::Cached => run_cached_prompt(&repo_id_port),
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
                contexts::status_cache::infrastructure::daemon_lifecycle::DaemonLifecycle;
            match subcommand {
                DaemonCommand::Start => {
                    contexts::status_cache::interface::cli::daemon::start(&lifecycle);
                }
                DaemonCommand::Stop => {
                    contexts::status_cache::interface::cli::daemon::stop(&lifecycle);
                }
                DaemonCommand::Status => {
                    contexts::status_cache::interface::cli::daemon::status(&lifecycle);
                }
                DaemonCommand::Refresh { repo_id } => run_daemon_refresh(&repo_id),
            }
        }
        None => {
            let _ = Cli::command().print_help();
        }
    }
}
