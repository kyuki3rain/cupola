use std::path::PathBuf;
use std::time::Duration;

use crate::domain::author_association::TrustedAssociations;
use crate::domain::claude_code_env_config::ClaudeCodeEnvConfig;
use crate::domain::claude_code_permissions_config::ClaudeCodePermissionsConfig;
use crate::domain::model_config::ModelConfig;

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
    pub max_ci_fix_cycles: u32,
    pub stall_timeout_secs: u64,
    pub log_level: LogLevel,
    pub log_dir: PathBuf,
    pub max_concurrent_sessions: Option<u32>,
    pub models: ModelConfig,
    pub trusted_associations: TrustedAssociations,
    pub trusted_reviewers: Vec<String>,
    /// Graceful shutdown のタイムアウト。
    /// `None` = 全セッション完了まで無限待機（shutdown_timeout_secs = 0 の場合）。
    /// `Some(d)` = 指定秒数後に強制終了。デフォルト 300 秒。
    pub shutdown_timeout: Option<Duration>,
    pub claude_code_env: ClaudeCodeEnvConfig,
    pub claude_code_permissions: ClaudeCodePermissionsConfig,
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
            max_ci_fix_cycles: 3,
            stall_timeout_secs: 1800,
            log_level: LogLevel::Info,
            log_dir: PathBuf::from(".cupola/logs"),
            max_concurrent_sessions: Some(3),
            models: ModelConfig::new_default("sonnet".to_string()),
            trusted_associations: TrustedAssociations::default(),
            trusted_reviewers: vec!["copilot-pull-request-reviewer".to_string()],
            shutdown_timeout: Some(Duration::from_secs(300)),
            claude_code_env: ClaudeCodeEnvConfig::default(),
            claude_code_permissions: ClaudeCodePermissionsConfig::default(),
        }
    }

    /// コメントの trust を統合判定する。
    /// `trusted_associations`（ロールベース）OR `trusted_reviewers`（ユーザー名ベース）。
    pub fn is_comment_trusted(
        &self,
        author_association: &crate::domain::author_association::AuthorAssociation,
        author_login: &str,
    ) -> bool {
        self.trusted_associations.is_trusted(author_association)
            || self.trusted_reviewers.iter().any(|r| r == author_login)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.log_dir.as_os_str().is_empty() {
            return Err("log_dir must not be empty".to_string());
        }
        if self.owner.is_empty() {
            return Err("owner must not be empty".to_string());
        }
        if self.repo.is_empty() {
            return Err("repo must not be empty".to_string());
        }
        if self.default_branch.is_empty() {
            return Err("default_branch must not be empty".to_string());
        }
        if self.language.is_empty() {
            return Err("language must not be empty".to_string());
        }
        if self.polling_interval_secs < 10 {
            return Err("polling_interval_secs must be at least 10".to_string());
        }
        if self.stall_timeout_secs < 60 {
            return Err("stall_timeout_secs must be at least 60".to_string());
        }
        if self.stall_timeout_secs <= self.polling_interval_secs {
            return Err(
                "stall_timeout_secs must be greater than polling_interval_secs".to_string(),
            );
        }
        if let Some(0) = self.max_concurrent_sessions {
            return Err("max_concurrent_sessions must be greater than 0".to_string());
        }
        if self.max_ci_fix_cycles == 0 {
            return Err("max_ci_fix_cycles must be greater than 0".to_string());
        }
        if self.max_retries == 0 {
            return Err("max_retries must be greater than 0".to_string());
        }
        self.models.validate()?;
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
        assert_eq!(config.log_dir, PathBuf::from(".cupola/logs"));
        assert_eq!(config.max_concurrent_sessions, Some(3));
        assert_eq!(config.models.default_model, "sonnet");
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
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.max_concurrent_sessions = None;
        assert!(config.validate().is_ok());
    }

    // Task 2.0: log_dir validation tests
    #[test]
    fn validate_rejects_empty_log_dir() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.log_dir = PathBuf::from("");
        assert_eq!(
            config.validate().unwrap_err().to_string(),
            "log_dir must not be empty"
        );
    }

    #[test]
    fn validate_accepts_nonempty_log_dir() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.log_dir = PathBuf::from(".cupola/logs");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn default_with_repo_log_dir_is_default_path() {
        let config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        assert_eq!(config.log_dir, PathBuf::from(".cupola/logs"));
    }

    // Task 2.1: String field empty checks
    #[test]
    fn validate_rejects_empty_owner() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.owner = String::new();
        let err = config.validate().unwrap_err();
        assert_eq!(err, "owner must not be empty");
    }

    #[test]
    fn validate_rejects_empty_repo() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.repo = String::new();
        let err = config.validate().unwrap_err();
        assert_eq!(err, "repo must not be empty");
    }

    #[test]
    fn validate_rejects_empty_default_branch() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.default_branch = String::new();
        let err = config.validate().unwrap_err();
        assert_eq!(err, "default_branch must not be empty");
    }

    #[test]
    fn validate_rejects_empty_language() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.language = String::new();
        let err = config.validate().unwrap_err();
        assert_eq!(err, "language must not be empty");
    }

    #[test]
    fn validate_rejects_empty_default_model() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.models.default_model = String::new();
        assert!(config.validate().is_err());
    }

    // Task 2.2: polling_interval_secs validation tests
    #[test]
    fn validate_rejects_polling_below_minimum() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.polling_interval_secs = 9;
        let err = config.validate().unwrap_err();
        assert_eq!(err, "polling_interval_secs must be at least 10");
    }

    #[test]
    fn validate_rejects_polling_zero() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.polling_interval_secs = 0;
        let err = config.validate().unwrap_err();
        assert_eq!(err, "polling_interval_secs must be at least 10");
    }

    #[test]
    fn validate_accepts_polling_at_minimum() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.polling_interval_secs = 10;
        // stall_timeout_secs must be > polling_interval_secs, default is 1800
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_accepts_polling_above_minimum() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.polling_interval_secs = 11;
        assert!(config.validate().is_ok());
    }

    // Task 2.3: stall_timeout_secs validation and correlation tests
    #[test]
    fn validate_rejects_stall_below_minimum() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.stall_timeout_secs = 59;
        let err = config.validate().unwrap_err();
        assert_eq!(err, "stall_timeout_secs must be at least 60");
    }

    #[test]
    fn validate_rejects_stall_typical_misconfig() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.stall_timeout_secs = 30;
        let err = config.validate().unwrap_err();
        assert_eq!(err, "stall_timeout_secs must be at least 60");
    }

    #[test]
    fn validate_accepts_stall_at_minimum_with_smaller_polling() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.stall_timeout_secs = 60;
        config.polling_interval_secs = 10;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_stall_equal_to_polling() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.polling_interval_secs = 60;
        config.stall_timeout_secs = 60;
        let err = config.validate().unwrap_err();
        assert_eq!(
            err,
            "stall_timeout_secs must be greater than polling_interval_secs"
        );
    }

    #[test]
    fn validate_rejects_stall_less_than_polling() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.polling_interval_secs = 120;
        config.stall_timeout_secs = 60;
        // stall=60 >= 60, but stall <= polling -> correlation error
        let err = config.validate().unwrap_err();
        assert_eq!(
            err,
            "stall_timeout_secs must be greater than polling_interval_secs"
        );
    }

    #[test]
    fn validate_accepts_stall_greater_than_polling() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.polling_interval_secs = 60;
        config.stall_timeout_secs = 61;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_skips_correlation_check_when_polling_too_small() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.polling_interval_secs = 5;
        // Even though stall_timeout_secs > polling_interval_secs, polling absolute check fires first
        let err = config.validate().unwrap_err();
        assert_eq!(err, "polling_interval_secs must be at least 10");
    }

    #[test]
    fn validate_accepts_valid_default_config() {
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn default_with_repo_max_ci_fix_cycles_is_3() {
        let config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        assert_eq!(config.max_ci_fix_cycles, 3);
    }

    #[test]
    fn validate_rejects_zero_max_ci_fix_cycles() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.max_ci_fix_cycles = 0;
        let err = config.validate().unwrap_err();
        assert_eq!(err, "max_ci_fix_cycles must be greater than 0");
    }

    #[test]
    fn validate_accepts_positive_max_ci_fix_cycles() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.max_ci_fix_cycles = 5;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_zero_max_retries() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.max_retries = 0;
        let err = config.validate().unwrap_err();
        assert_eq!(err, "max_retries must be greater than 0");
    }

    #[test]
    fn validate_accepts_positive_max_retries() {
        let mut config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        config.max_retries = 1;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_accepts_default_max_retries() {
        let config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        assert_eq!(config.max_retries, 3);
        assert!(config.validate().is_ok());
    }
}
