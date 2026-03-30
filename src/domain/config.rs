use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub owner: String,
    pub repo: String,
    pub default_branch: String,
    pub language: String,
    pub polling_interval_secs: u64,
    pub max_retries: u32,
    pub stall_timeout_secs: u64,
    pub log_level: LogLevel,
    pub log_dir: Option<PathBuf>,
    pub max_concurrent_sessions: Option<u32>,
    pub model: String,
}

impl Config {
    pub fn default_with_repo(owner: String, repo: String, default_branch: String) -> Self {
        Self {
            owner,
            repo,
            default_branch,
            language: "ja".to_string(),
            polling_interval_secs: 60,
            max_retries: 3,
            stall_timeout_secs: 1800,
            log_level: LogLevel::Info,
            log_dir: None,
            max_concurrent_sessions: None,
            model: "sonnet".to_string(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(0) = self.max_concurrent_sessions {
            return Err("max_concurrent_sessions must be greater than 0".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_with_repo_sets_correct_defaults() {
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        assert_eq!(config.owner, "owner");
        assert_eq!(config.repo, "repo");
        assert_eq!(config.default_branch, "main");
        assert_eq!(config.language, "ja");
        assert_eq!(config.polling_interval_secs, 60);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.stall_timeout_secs, 1800);
        assert_eq!(config.log_level, LogLevel::Info);
        assert!(config.log_dir.is_none());
        assert!(config.max_concurrent_sessions.is_none());
        assert_eq!(config.model, "sonnet");
    }

    #[test]
    fn validate_rejects_zero_max_concurrent_sessions() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.max_concurrent_sessions = Some(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_accepts_positive_max_concurrent_sessions() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.max_concurrent_sessions = Some(3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_accepts_none_max_concurrent_sessions() {
        let config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        assert!(config.validate().is_ok());
    }
}
