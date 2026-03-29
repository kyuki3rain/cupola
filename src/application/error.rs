#[derive(Debug, thiserror::Error)]
pub enum CupolaError {
    #[error("domain error: {0}")]
    Domain(String),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("github error: {0}")]
    GitHub(String),

    #[error("claude code error: {0}")]
    ClaudeCode(String),

    #[error("git error: {0}")]
    Git(String),

    #[error("config error: {0}")]
    Config(String),
}

impl CupolaError {
    pub fn github(msg: impl Into<String>) -> Self {
        Self::GitHub(msg.into())
    }

    pub fn claude_code(msg: impl Into<String>) -> Self {
        Self::ClaudeCode(msg.into())
    }

    pub fn git(msg: impl Into<String>) -> Self {
        Self::Git(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_error_display() {
        let err = CupolaError::Domain("invalid state".into());
        assert_eq!(err.to_string(), "domain error: invalid state");
    }

    #[test]
    fn github_error_display() {
        let err = CupolaError::github("rate limited");
        assert_eq!(err.to_string(), "github error: rate limited");
    }

    #[test]
    fn claude_code_error_display() {
        let err = CupolaError::claude_code("process crashed");
        assert_eq!(err.to_string(), "claude code error: process crashed");
    }

    #[test]
    fn git_error_display() {
        let err = CupolaError::git("merge conflict");
        assert_eq!(err.to_string(), "git error: merge conflict");
    }

    #[test]
    fn config_error_display() {
        let err = CupolaError::config("missing owner field");
        assert_eq!(err.to_string(), "config error: missing owner field");
    }

    #[test]
    fn database_error_from_rusqlite() {
        let rusqlite_err = rusqlite::Error::QueryReturnedNoRows;
        let err: CupolaError = rusqlite_err.into();
        assert!(matches!(err, CupolaError::Database(_)));
        assert!(err.to_string().contains("database error"));
    }
}
