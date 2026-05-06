use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::prompt::Prompt;

pub trait PromptRepository {
    /// # Errors
    /// Returns `RepositoryError` if the PR state cannot be fetched due to infrastructure failure.
    fn fetch(&self) -> Result<Prompt, RepositoryError>;
}
