mod cached;
mod refresh;

use clap::{CommandFactory, Parser, Subcommand};
use configuration_domain::config::TokenConfig;
use configuration_domain::repository::ConfigRepository as _;
use configuration_infrastructure::toml_loader::TomlConfigRepository;
use merge_readiness_application::{
    OutputToken,
    errors::ErrorToken,
    prompt::{ExecutionMode, RepoIdPort},
};
use merge_readiness_infrastructure::{gh::GhClient, logger::Logger};
use merge_readiness_interface::{
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
        merge_readiness_infrastructure::repo_id::get()
    }
}

pub(crate) struct ConfigAdapter(configuration_domain::config::Config);

impl ConfigAdapter {
    pub(crate) fn load() -> Self {
        Self(TomlConfigRepository.load())
    }
}

impl PresentationConfigPort for ConfigAdapter {
    fn render_token(&self, token: &OutputToken) -> String {
        let e = TokenConfig::default();
        match token {
            OutputToken::MergeReady => self.0.merge_ready.as_ref().unwrap_or(&e).render("✓", "merge-ready"),
            OutputToken::Conflict => self.0.conflict.as_ref().unwrap_or(&e).render("✗", "conflict"),
            OutputToken::UpdateBranch => self.0.update_branch.as_ref().unwrap_or(&e).render("✗", "update-branch"),
            OutputToken::SyncUnknown => self.0.sync_unknown.as_ref().unwrap_or(&e).render("?", "sync-unknown"),
            OutputToken::CiFail => self.0.ci_fail.as_ref().unwrap_or(&e).render("✗", "ci-fail"),
            OutputToken::CiAction => self.0.ci_action.as_ref().unwrap_or(&e).render("⚠", "ci-action"),
            OutputToken::ReviewRequested => self.0.review.as_ref().unwrap_or(&e).render("⚠", "review"),
        }
    }

    fn render_error_token(&self, token: ErrorToken) -> String {
        let e = TokenConfig::default();
        let err = self.0.error.as_ref();
        match token {
            ErrorToken::AuthRequired => err.and_then(|e| e.auth_required.as_ref()).unwrap_or(&e).render("!", "gh auth login"),
            ErrorToken::RateLimited => err.and_then(|e| e.rate_limited.as_ref()).unwrap_or(&e).render("✗", "rate-limited"),
            ErrorToken::ApiError => err.and_then(|e| e.api_error.as_ref()).unwrap_or(&e).render("✗", "api-error"),
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
            merge_readiness_interface::cli::prompt::direct::run(
                &GhClient::new(),
                &Logger,
                ConfigAdapter::load(),
            );
        }
        ExecutionMode::Cached => cached::run(),
        ExecutionMode::BackgroundRefresh { repo_id } => refresh::run(&repo_id),
    }
}
