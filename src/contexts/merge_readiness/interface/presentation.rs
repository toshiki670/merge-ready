use crate::contexts::merge_readiness::application::{
    OutputToken,
    errors::{ErrorPresenter, ErrorToken},
};

pub trait PresentationConfigPort {
    fn render_token(&self, token: &OutputToken) -> String;
    fn render_error_token(&self, token: ErrorToken) -> String;
}

pub struct Presenter<C: PresentationConfigPort> {
    config_port: C,
}

impl<C: PresentationConfigPort> Presenter<C> {
    pub fn new(config_port: C) -> Self {
        Self { config_port }
    }

    pub fn render_to_string(&self, tokens: &[OutputToken]) -> String {
        tokens
            .iter()
            .map(|t| self.config_port.render_token(t))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn display(&self, tokens: &[OutputToken]) {
        print!("{}", self.render_to_string(tokens));
    }
}

impl<C: PresentationConfigPort> ErrorPresenter for Presenter<C> {
    fn show_error(&self, token: ErrorToken) {
        print!("{}", self.config_port.render_error_token(token));
    }
}
