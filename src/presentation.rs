use crate::application::{
    OutputToken,
    errors::{ErrorPresenter, ErrorToken},
};

/// `OutputToken` を表示文字列に変換する
fn render(token: &OutputToken) -> &'static str {
    match token {
        OutputToken::Conflict => "✗ conflict",
        OutputToken::UpdateBranch => "✗ update-branch",
        OutputToken::SyncUnknown => "? sync-unknown",
        OutputToken::CiFail => "✗ ci-fail",
        OutputToken::CiAction => "⚠ ci-action",
        OutputToken::ReviewRequested => "⚠ review",
        OutputToken::MergeReady => "✓ merge-ready",
    }
}

/// `ErrorToken` を表示文字列に変換する
fn render_error(token: ErrorToken) -> &'static str {
    match token {
        ErrorToken::AuthRequired => "! gh auth login",
        ErrorToken::RateLimited => "✗ rate-limited",
        ErrorToken::ApiError => "✗ api-error",
    }
}

/// トークン列を `stdout` に出力する（末尾改行なし）
pub fn display(tokens: &[OutputToken]) {
    let rendered: Vec<&str> = tokens.iter().map(render).collect();
    print!("{}", rendered.join(" "));
}

pub struct Presenter;

impl ErrorPresenter for Presenter {
    fn show_error(&self, token: ErrorToken) {
        print!("{}", render_error(token));
    }
}
