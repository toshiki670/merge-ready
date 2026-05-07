use crate::contexts::evaluation::domain::error::RepositoryError;

pub(super) enum GhError {
    NotInstalled,
    AuthRequired,
    NoPr,
    RateLimited,
    Timeout,
    NotGithubRepository,
    ApiError(String),
}

impl From<GhError> for RepositoryError {
    fn from(e: GhError) -> Self {
        match e {
            GhError::NotInstalled | GhError::AuthRequired => RepositoryError::Unauthenticated,
            GhError::RateLimited => RepositoryError::RateLimited,
            GhError::NoPr
            | GhError::NotGithubRepository
            | GhError::Timeout
            | GhError::ApiError(_) => RepositoryError::Unexpected,
        }
    }
}

pub(super) fn classify_gh_error(exit_code: i32, stderr: &str) -> GhError {
    if exit_code == 4 || (exit_code == 1 && stderr.contains("HTTP 401")) {
        GhError::AuthRequired
    } else if exit_code == 1 && stderr.contains("no pull requests found") {
        GhError::NoPr
    } else if exit_code == 1 && stderr.contains("rate limit") {
        GhError::RateLimited
    } else if exit_code == 1
        && (stderr.contains("no git remotes found")
            || stderr.contains(
                "none of the git remotes configured for this repository point to a known GitHub host",
            ))
    {
        GhError::NotGithubRepository
    } else {
        GhError::ApiError(stderr.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_auth_exit_code() {
        assert!(matches!(classify_gh_error(4, ""), GhError::AuthRequired));
    }

    #[test]
    fn classify_no_pr_message() {
        assert!(matches!(
            classify_gh_error(1, "no pull requests found"),
            GhError::NoPr
        ));
    }

    #[test]
    fn classify_rate_limit_message() {
        assert!(matches!(
            classify_gh_error(1, "API rate limit exceeded"),
            GhError::RateLimited
        ));
    }

    #[test]
    fn classify_non_github_remote_message() {
        assert!(matches!(
            classify_gh_error(1, "no git remotes found"),
            GhError::NotGithubRepository
        ));
    }
}
