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
            ConfigCommand::Edit => run_config_edit(),
            ConfigCommand::Update => run_config_update(),
        },
        None => {
            let _ = Cli::command().print_help();
        }
    }
}

fn run_config_edit() {
    use std::ffi::OsString;

    let editor = std::env::var_os("VISUAL")
        .or_else(|| std::env::var_os("EDITOR"))
        .unwrap_or_else(|| OsString::from("vi"));

    let Some(path) = contexts::configuration::infrastructure::toml_loader::config_path() else {
        return;
    };

    ensure_config_file(&path);
    let _ = std::process::Command::new(editor).arg(&path).status();
}

fn run_config_update() {
    use contexts::configuration::domain::config::{CURRENT_VERSION, Config};

    let Some(path) = contexts::configuration::infrastructure::toml_loader::config_path() else {
        return;
    };

    if !path.exists() {
        ensure_config_file(&path);
        return;
    }

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut config: Config = toml::from_str(&content).unwrap_or_default();

    if config.version == CURRENT_VERSION {
        return;
    }

    config.version = CURRENT_VERSION;
    config.fill_defaults();
    if let Ok(new_content) = toml::to_string_pretty(&config) {
        let _ = std::fs::write(&path, new_content);
    }
}

fn ensure_config_file(path: &std::path::Path) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, default_config_toml());
}

fn default_config_toml() -> &'static str {
    "\
# merge-ready configuration
# All fields are optional; omit a section to use built-in defaults.

# [merge_ready]
# symbol = \"✓\"
# label = \"merge-ready\"
# format = \"$symbol $label\"

# [conflict]
# symbol = \"✗\"
# label = \"conflict\"

# [update_branch]
# symbol = \"✗\"
# label = \"update-branch\"

# [sync_unknown]
# symbol = \"?\"
# label = \"sync-unknown\"

# [ci_fail]
# symbol = \"✗\"
# label = \"ci-fail\"

# [ci_action]
# symbol = \"⚠\"
# label = \"ci-action\"

# [review]
# symbol = \"⚠\"
# label = \"review\"
"
}
