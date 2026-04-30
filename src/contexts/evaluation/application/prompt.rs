pub mod display_item;

use super::errors::{ErrorToken, into_token};
use super::port::ErrorLogger;
use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::pr_state::PrRepository;
use crate::contexts::evaluation::domain::pr_state::PrState;

/// PR 状態を取得するユースケース。
/// `NotFound` は表示不要な状態として空リストを返す。
///
/// # Errors
///
/// リポジトリの取得に失敗し、エラートークンに変換できた場合に `Err(ErrorToken)` を返す。
pub fn fetch<R, L>(
    repo: &R,
    logger: &L,
) -> Result<(Vec<display_item::DisplayItem>, bool), ErrorToken>
where
    R: PrRepository,
    L: ErrorLogger,
{
    let state = match repo.fetch() {
        Ok(s) => s,
        Err(RepositoryError::NotFound) => PrState::NoPr,
        Err(e) => match into_token(e, logger) {
            Some(token) => return Err(token),
            None => {
                unreachable!("into_token returns None only for NotFound, which is handled above")
            }
        },
    };
    let is_terminal = state.is_terminal();
    Ok((display_item::from_pr_state(state), is_terminal))
}
