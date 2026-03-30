use std::path::Path;

use serde::Deserialize;

use crate::domain::config::{Config, LogLevel};

#[derive(Debug, Deserialize)]
pub struct CupolaToml {
    pub owner: String,
    pub repo: String,
    pub default_branch: String,
    pub language: Option<String>,
    pub polling_interval_secs: Option<u64>,
    pub max_retries: Option<u32>,
    pub stall_timeout_secs: Option<u64>,
    pub max_concurrent_sessions: Option<u32>,
    pub model: Option<String>,
    pub log: Option<LogToml>,
}

#[derive(Debug, Deserialize)]
pub struct LogToml {
    pub level: Option<String>,
    pub dir: Option<String>,
}

/// CLI overrides that take priority over cupola.toml values.
pub struct CliOverrides {
    pub polling_interval_secs: Option<u64>,
    pub log_level: Option<String>,
}

impl CupolaToml {
    pub fn into_config(self, overrides: &CliOverrides) -> Config {
        let log_level = overrides
            .log_level
            .as_deref()
            .or(self.log.as_ref().and_then(|l| l.level.as_deref()))
            .map(parse_log_level)
            .unwrap_or(LogLevel::Info);

        let log_dir = self
            .log
            .as_ref()
            .and_then(|l| l.dir.as_deref())
            .map(Into::into);

        Config {
            owner: self.owner,
            repo: self.repo,
            default_branch: self.default_branch,
            language: self.language.unwrap_or_else(|| "ja".to_string()),
            polling_interval_secs: overrides
                .polling_interval_secs
                .or(self.polling_interval_secs)
                .unwrap_or(60),
            max_retries: self.max_retries.unwrap_or(3),
            stall_timeout_secs: self.stall_timeout_secs.unwrap_or(1800),
            log_level,
            log_dir,
            max_concurrent_sessions: self.max_concurrent_sessions,
            model: self.model.unwrap_or_else(|| "sonnet".to_string()),
        }
    }
}

fn parse_log_level(s: &str) -> LogLevel {
    match s.to_lowercase().as_str() {
        "trace" => LogLevel::Trace,
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warn" => LogLevel::Warn,
        "error" => LogLevel::Error,
        _ => LogLevel::Info,
    }
}

pub fn load_toml(path: &Path) -> Result<CupolaToml, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadFailed {
        path: path.display().to_string(),
        source: e,
    })?;
    toml::from_str(&content).map_err(|e| ConfigError::ParseFailed {
        path: path.display().to_string(),
        source: e,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config from {path}: {source}")]
    ReadFailed {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse config at {path}: {source}")]
    ParseFailed {
        path: String,
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_toml() {
        let toml_str = r#"
owner = "kyuki3rain"
repo = "my-project"
default_branch = "main"
language = "en"
polling_interval_secs = 30
max_retries = 5
stall_timeout_secs = 900

[log]
level = "debug"
dir = ".cupola/logs"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        assert_eq!(parsed.owner, "kyuki3rain");
        assert_eq!(parsed.repo, "my-project");
        assert_eq!(parsed.default_branch, "main");
        assert_eq!(parsed.language.as_deref(), Some("en"));
        assert_eq!(parsed.polling_interval_secs, Some(30));
        assert_eq!(parsed.max_retries, Some(5));
        assert_eq!(parsed.stall_timeout_secs, Some(900));
        assert!(parsed.max_concurrent_sessions.is_none());
        assert_eq!(
            parsed.log.as_ref().and_then(|l| l.level.as_deref()),
            Some("debug")
        );
        assert_eq!(
            parsed.log.as_ref().and_then(|l| l.dir.as_deref()),
            Some(".cupola/logs")
        );
    }

    #[test]
    fn parse_minimal_toml() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        assert_eq!(parsed.owner, "user");
        assert!(parsed.language.is_none());
        assert!(parsed.polling_interval_secs.is_none());
        assert!(parsed.log.is_none());
    }

    #[test]
    fn missing_required_field_fails() {
        let toml_str = r#"
owner = "user"
repo = "repo"
"#;
        let result: Result<CupolaToml, _> = toml::from_str(toml_str);
        assert!(result.is_err(), "missing default_branch should fail");
    }

    #[test]
    fn into_config_uses_defaults_for_optional_fields() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides);

        assert_eq!(config.owner, "user");
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
    fn parse_max_concurrent_sessions() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
max_concurrent_sessions = 3
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        assert_eq!(parsed.max_concurrent_sessions, Some(3));

        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides);
        assert_eq!(config.max_concurrent_sessions, Some(3));
    }

    #[test]
    fn cli_overrides_take_priority() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
polling_interval_secs = 60

[log]
level = "info"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: Some(10),
            log_level: Some("debug".to_string()),
        };
        let config = parsed.into_config(&overrides);

        assert_eq!(config.polling_interval_secs, 10);
        assert_eq!(config.log_level, LogLevel::Debug);
    }

    #[test]
    fn parse_model_specified() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
model = "opus"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        assert_eq!(parsed.model.as_deref(), Some("opus"));
    }

    #[test]
    fn parse_model_unspecified() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        assert!(parsed.model.is_none());
    }

    #[test]
    fn into_config_model_specified() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
model = "opus"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides);
        assert_eq!(config.model, "opus");
    }

    #[test]
    fn into_config_model_default() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides);
        assert_eq!(config.model, "sonnet");
    }

    #[test]
    fn load_toml_file_not_found() {
        let result = load_toml(Path::new("/nonexistent/path.toml"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::ReadFailed { .. }),
            "expected ReadFailed, got {err:?}"
        );
    }
}
