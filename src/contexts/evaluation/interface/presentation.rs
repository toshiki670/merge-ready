use crate::contexts::evaluation::application::config_service::ConfigService;
use crate::contexts::evaluation::application::{
    OutputToken,
    errors::{ErrorPresenter, ErrorToken},
};

pub struct Presenter {
    config: ConfigService,
}

impl Presenter {
    pub fn new(config: ConfigService) -> Self {
        Self { config }
    }

    pub fn render_output(&self, tokens: &[OutputToken], error: Option<ErrorToken>) -> String {
        if let Some(err) = error {
            self.render_error_token(err)
        } else {
            self.render_to_string(tokens)
        }
    }

    pub fn render_to_string(&self, tokens: &[OutputToken]) -> String {
        tokens
            .iter()
            .map(|t| self.render_output_token(t))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn render_output_token(&self, token: &OutputToken) -> String {
        match token {
            OutputToken::MergeReady => self.config.render_merge_ready(),
            OutputToken::Conflict => self.config.render_conflict(),
            OutputToken::UpdateBranch => self.config.render_update_branch(),
            OutputToken::SyncUnknown => self.config.render_sync_unknown(),
            OutputToken::CiFail => self.config.render_ci_fail(),
            OutputToken::CiAction => self.config.render_ci_action(),
            OutputToken::ReviewRequested => self.config.render_review(),
        }
    }

    fn render_error_token(&self, token: ErrorToken) -> String {
        match token {
            ErrorToken::AuthRequired => self.config.render_auth_required(),
            ErrorToken::RateLimited => self.config.render_rate_limited(),
            ErrorToken::ApiError => self.config.render_api_error(),
        }
    }
}

impl ErrorPresenter for Presenter {
    fn show_error(&self, token: ErrorToken) {
        print!("{}", self.render_error_token(token));
    }
}
