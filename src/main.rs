mod cached;
mod contexts;
mod refresh;

use clap::{CommandFactory, Parser, Subcommand};
use contexts::configuration::domain::config::TokenConfig;
use contexts::configuration::domain::repository::ConfigRepository as _;
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
}

struct InfraRepoIdPort;

impl RepoIdPort for InfraRepoIdPort {
    fn get(&self) -> Option<String> {
        contexts::merge_readiness::infrastructure::repo_id::get()
    }
}

pub(crate) struct ConfigAdapter(contexts::configuration::domain::config::Config);

impl ConfigAdapter {
    pub(crate) fn load() -> Self {
        Self(TomlConfigRepository.load())
    }
}

impl PresentationConfigPort for ConfigAdapter {
    fn render_token(&self, token: &OutputToken) -> String {
        let default = TokenConfig::default();
        match token {
            OutputToken::MergeReady => self
                .0
                .merge_ready
                .as_ref()
                .unwrap_or(&default)
                .render("✓", "merge-ready"),
            OutputToken::Conflict => self
                .0
                .conflict
                .as_ref()
                .unwrap_or(&default)
                .render("✗", "conflict"),
            OutputToken::UpdateBranch => self
                .0
                .update_branch
                .as_ref()
                .unwrap_or(&default)
                .render("✗", "update-branch"),
            OutputToken::SyncUnknown => self
                .0
                .sync_unknown
                .as_ref()
                .unwrap_or(&default)
                .render("?", "sync-unknown"),
            OutputToken::CiFail => self
                .0
                .ci_fail
                .as_ref()
                .unwrap_or(&default)
                .render("✗", "ci-fail"),
            OutputToken::CiAction => self
                .0
                .ci_action
                .as_ref()
                .unwrap_or(&default)
                .render("⚠", "ci-action"),
            OutputToken::ReviewRequested => self
                .0
                .review
                .as_ref()
                .unwrap_or(&default)
                .render("⚠", "review"),
        }
    }

    fn render_error_token(&self, token: ErrorToken) -> String {
        let default = TokenConfig::default();
        let err = self.0.error.as_ref();
        match token {
            ErrorToken::AuthRequired => err
                .and_then(|ec| ec.auth_required.as_ref())
                .unwrap_or(&default)
                .render("!", "gh auth login"),
            ErrorToken::RateLimited => err
                .and_then(|ec| ec.rate_limited.as_ref())
                .unwrap_or(&default)
                .render("✗", "rate-limited"),
            ErrorToken::ApiError => err
                .and_then(|ec| ec.api_error.as_ref())
                .unwrap_or(&default)
                .render("✗", "api-error"),
        }
    }
}

fn main() {
    let repo_id_port = InfraRepoIdPort;
    let Some(mode) = (match Cli::parse().command {
        Some(Command::Prompt(args)) => prompt::resolve_mode(&args, &repo_id_port),
        None => {
            let _ = Cli::command().print_help();
            None
        }
    }) else {
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
