use super::errors::{ErrorToken, into_token};
use super::port::ErrorLogger;
use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::pr_state::PrRepository;
use crate::contexts::evaluation::domain::pr_state::PrState;

/// PR 状態を取得するユースケース。
/// `NotFound` は表示不要な状態として `Ok(Unknown)` を返す。
pub fn fetch<R, L>(repo: &R, logger: &L) -> Result<(PrState, bool), ErrorToken>
where
    R: PrRepository,
    L: ErrorLogger,
{
    let state = match repo.fetch() {
        Ok(s) => s,
        Err(RepositoryError::NotFound) => PrState::Unknown,
        Err(e) => match into_token(e, logger) {
            Some(token) => return Err(token),
            None => PrState::Unknown,
        },
    };
    let is_terminal = state.is_terminal();
    Ok((state, is_terminal))
}
