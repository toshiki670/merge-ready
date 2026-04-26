use std::sync::Mutex;

use super::OutputToken;
use super::errors::{ErrorLogger, ErrorPresenter, ErrorToken};
use crate::contexts::evaluation::domain::pr_state::PrRepository;

/// gh を呼んで出力トークン・エラートークン・終端判定を返す。
///
/// `daemon refresh` 処理用。エラーは stderr に出力せず内部で捕捉する。
/// `is_terminal` が `true` のとき daemon はポーリングを停止してよい。
pub fn fetch_output<R, L>(repo: &R, logger: &L) -> (Vec<OutputToken>, Option<ErrorToken>, bool)
where
    R: PrRepository,
    L: ErrorLogger,
{
    struct CapturingPresenter(Mutex<Option<ErrorToken>>);

    impl ErrorPresenter for CapturingPresenter {
        fn show_error(&self, token: ErrorToken) {
            *self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(token);
        }
    }

    let presenter = CapturingPresenter(Mutex::new(None));

    let pr_state = match repo.fetch() {
        Ok(s) => s,
        Err(e) => {
            super::errors::handle(e, logger, &presenter);
            let error = presenter
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            return (vec![], error, false);
        }
    };

    let is_terminal = pr_state.is_terminal();
    let tokens = super::map_pr_state_to_tokens(pr_state);
    let error = presenter
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();

    (tokens, error, is_terminal)
}
