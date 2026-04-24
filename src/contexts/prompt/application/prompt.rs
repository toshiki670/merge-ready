use std::sync::Mutex;

use super::OutputToken;
use super::errors::{ErrorLogger, ErrorPresenter, ErrorToken};
use super::port::PromptStatusPort;

/// gh を呼んで出力トークンとエラートークンを返す。
///
/// `daemon refresh` 処理用。エラーは stderr に出力せず内部で捕捉する。
/// エラー発生時は `Option<ErrorToken>` に値が入り、daemon がキャッシュに書き込める。
pub fn fetch_output<C, L>(client: &C, logger: &L) -> (Vec<OutputToken>, Option<ErrorToken>)
where
    C: PromptStatusPort + Sync,
    L: ErrorLogger + Sync,
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
    let tokens = super::run(client, logger, &presenter);
    let error = presenter
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();

    (tokens, error)
}
