use std::path::Path;

use serde::Deserialize;

use crate::domain::author_association::{AuthorAssociation, TrustedAssociations};
use crate::domain::config::{Config, LogLevel};
use crate::domain::model_config::{ModelConfig, PerPhaseModels, WeightModelConfig};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ModelTier {
    Uniform(String),
    PerPhase {
        design: Option<String>,
        design_fix: Option<String>,
        implementation: Option<String>,
        implementation_fix: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct ModelsToml {
    light: Option<ModelTier>,
    medium: Option<ModelTier>,
    heavy: Option<ModelTier>,
}

fn model_tier_to_weight_config(tier: ModelTier) -> WeightModelConfig {
    match tier {
        ModelTier::Uniform(s) => WeightModelConfig::Uniform(s),
        ModelTier::PerPhase {
            design,
            design_fix,
            implementation,
            implementation_fix,
        } => WeightModelConfig::PerPhase(PerPhaseModels {
            design,
            design_fix,
            implementation,
            implementation_fix,
        }),
    }
}

#[derive(Debug, Deserialize)]
pub struct CupolaToml {
    pub owner: String,
    pub repo: String,
    pub default_branch: String,
    pub language: Option<String>,
    pub polling_interval_secs: Option<u64>,
    pub max_retries: Option<u32>,
    pub max_ci_fix_cycles: Option<u32>,
    pub stall_timeout_secs: Option<u64>,
    pub max_concurrent_sessions: Option<u32>,
    pub model: Option<String>,
    models: Option<ModelsToml>,
    pub log: Option<LogToml>,
    pub trusted_associations: Option<Vec<String>>,
    pub trusted_reviewers: Option<Vec<String>>,
    /// Graceful shutdown タイムアウト（秒）。
    /// 未設定: デフォルト 300 秒。
    /// 0: 無限待機。
    /// 正の整数: n 秒タイムアウト。
    pub shutdown_timeout_secs: Option<u64>,
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
    pub fn into_config(self, overrides: &CliOverrides) -> Result<Config, ConfigError> {
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
            .map(Into::into)
            .unwrap_or_else(|| std::path::PathBuf::from(".cupola/logs"));

        let default_model = self.model.unwrap_or_else(|| "sonnet".to_string());

        let models = if let Some(models_toml) = self.models {
            ModelConfig {
                default_model,
                light: models_toml.light.map(model_tier_to_weight_config),
                medium: models_toml.medium.map(model_tier_to_weight_config),
                heavy: models_toml.heavy.map(model_tier_to_weight_config),
            }
        } else {
            ModelConfig::new_default(default_model)
        };

        let trusted_associations = match &self.trusted_associations {
            None => TrustedAssociations::default(),
            Some(values) if values.len() == 1 && values[0].to_lowercase() == "all" => {
                TrustedAssociations::All
            }
            Some(values) => {
                let assocs = values
                    .iter()
                    .map(|s| {
                        s.parse::<AuthorAssociation>()
                            .map_err(|_| ConfigError::InvalidAssociation(s.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                TrustedAssociations::Specific(assocs)
            }
        };

        let shutdown_timeout = match self.shutdown_timeout_secs {
            None => Some(std::time::Duration::from_secs(300)), // デフォルト 300 秒
            Some(0) => None,                                   // 無限待機
            Some(n) => Some(std::time::Duration::from_secs(n)), // n 秒タイムアウト
        };

        Ok(Config {
            owner: self.owner,
            repo: self.repo,
            default_branch: self.default_branch,
            language: self.language.unwrap_or_else(|| "ja".to_string()),
            polling_interval_secs: overrides
                .polling_interval_secs
                .or(self.polling_interval_secs)
                .unwrap_or(60),
            max_retries: self.max_retries.unwrap_or(3),
            max_ci_fix_cycles: self.max_ci_fix_cycles.unwrap_or(3),
            stall_timeout_secs: self.stall_timeout_secs.unwrap_or(1800),
            log_level,
            log_dir,
            max_concurrent_sessions: Some(self.max_concurrent_sessions.unwrap_or(3)),
            models,
            trusted_associations,
            trusted_reviewers: self
                .trusted_reviewers
                .unwrap_or_else(|| vec!["copilot-pull-request-reviewer".to_string()]),
            shutdown_timeout,
        })
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
    #[error("invalid author association value: {0}")]
    InvalidAssociation(String),
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
        let config = parsed.into_config(&overrides).expect("should succeed");

        assert_eq!(config.owner, "user");
        assert_eq!(config.language, "ja");
        assert_eq!(config.polling_interval_secs, 60);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.stall_timeout_secs, 1800);
        assert_eq!(config.log_level, LogLevel::Info);
        assert_eq!(config.log_dir, std::path::PathBuf::from(".cupola/logs"));
        assert_eq!(config.max_concurrent_sessions, Some(3));
        assert_eq!(config.models.default_model, "sonnet");
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
        let config = parsed.into_config(&overrides).expect("should succeed");
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
        let config = parsed.into_config(&overrides).expect("should succeed");

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
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert_eq!(config.models.default_model, "opus");
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
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert_eq!(config.models.default_model, "sonnet");
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

    #[test]
    fn parse_models_section_uniform() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
model = "sonnet"

[models]
light = "haiku"
heavy = "opus"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        assert!(parsed.models.is_some());
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert_eq!(config.models.default_model, "sonnet");
        assert!(config.models.light.is_some());
        assert!(config.models.heavy.is_some());
        assert!(config.models.medium.is_none());
    }

    #[test]
    fn parse_models_section_per_phase() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
model = "sonnet"

[models.heavy]
design = "opus"
implementation = "opus"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        assert!(parsed.models.is_some());
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert_eq!(config.models.default_model, "sonnet");
        assert!(config.models.heavy.is_some());
    }

    #[test]
    fn fallback_chain_works() {
        use crate::domain::phase::Phase;
        use crate::domain::task_weight::TaskWeight;

        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
model = "sonnet"

[models]
heavy = "opus"
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");

        // Heavy with phase → opus (Uniform)
        assert_eq!(
            config
                .models
                .resolve(TaskWeight::Heavy, Some(Phase::Design)),
            "opus"
        );
        // Medium (no config) → sonnet (global default)
        assert_eq!(
            config
                .models
                .resolve(TaskWeight::Medium, Some(Phase::Design)),
            "sonnet"
        );
        // None phase → sonnet
        assert_eq!(config.models.resolve(TaskWeight::Heavy, None), "sonnet");
    }

    #[test]
    fn label_to_weight_both_labels_heavy_wins() {
        use crate::application::polling_use_case::label_to_weight;
        use crate::domain::task_weight::TaskWeight;

        let labels = vec!["weight:heavy".to_string(), "weight:light".to_string()];
        assert_eq!(label_to_weight(&labels), TaskWeight::Heavy);
    }

    #[test]
    fn label_to_weight_only_light() {
        use crate::application::polling_use_case::label_to_weight;
        use crate::domain::task_weight::TaskWeight;

        let labels = vec!["weight:light".to_string()];
        assert_eq!(label_to_weight(&labels), TaskWeight::Light);
    }

    #[test]
    fn label_to_weight_only_heavy() {
        use crate::application::polling_use_case::label_to_weight;
        use crate::domain::task_weight::TaskWeight;

        let labels = vec!["weight:heavy".to_string()];
        assert_eq!(label_to_weight(&labels), TaskWeight::Heavy);
    }

    #[test]
    fn into_config_max_ci_fix_cycles_default() {
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
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert_eq!(config.max_ci_fix_cycles, 3);
    }

    #[test]
    fn into_config_max_ci_fix_cycles_from_toml() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
max_ci_fix_cycles = 5
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert_eq!(config.max_ci_fix_cycles, 5);
    }

    #[test]
    fn into_config_max_ci_fix_cycles_zero_fails_validation() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
max_ci_fix_cycles = 0
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert!(config.validate().is_err());
        assert_eq!(
            config.validate().unwrap_err(),
            "max_ci_fix_cycles must be greater than 0"
        );
    }

    #[test]
    fn label_to_weight_no_labels_medium() {
        use crate::application::polling_use_case::label_to_weight;
        use crate::domain::task_weight::TaskWeight;

        let labels: Vec<String> = vec![];
        assert_eq!(label_to_weight(&labels), TaskWeight::Medium);
    }

    // --- trusted_associations TOML 変換テスト ---

    #[test]
    fn trusted_associations_omitted_uses_default() {
        use crate::domain::author_association::{AuthorAssociation, TrustedAssociations};

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
        let config = parsed.into_config(&overrides).expect("should succeed");
        // Default: Owner, Member, Collaborator
        assert!(
            config
                .trusted_associations
                .is_trusted(&AuthorAssociation::Owner)
        );
        assert!(
            config
                .trusted_associations
                .is_trusted(&AuthorAssociation::Member)
        );
        assert!(
            config
                .trusted_associations
                .is_trusted(&AuthorAssociation::Collaborator)
        );
        assert!(
            !config
                .trusted_associations
                .is_trusted(&AuthorAssociation::Contributor)
        );
        assert!(matches!(
            config.trusted_associations,
            TrustedAssociations::Specific(_)
        ));
    }

    #[test]
    fn trusted_associations_all_string() {
        use crate::domain::author_association::{AuthorAssociation, TrustedAssociations};

        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
trusted_associations = ["all"]
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert!(matches!(
            config.trusted_associations,
            TrustedAssociations::All
        ));
        assert!(
            config
                .trusted_associations
                .is_trusted(&AuthorAssociation::None)
        );
    }

    #[test]
    fn trusted_associations_list() {
        use crate::domain::author_association::{AuthorAssociation, TrustedAssociations};

        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
trusted_associations = ["OWNER", "CONTRIBUTOR"]
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert!(matches!(
            config.trusted_associations,
            TrustedAssociations::Specific(_)
        ));
        assert!(
            config
                .trusted_associations
                .is_trusted(&AuthorAssociation::Owner)
        );
        assert!(
            config
                .trusted_associations
                .is_trusted(&AuthorAssociation::Contributor)
        );
        assert!(
            !config
                .trusted_associations
                .is_trusted(&AuthorAssociation::Member)
        );
    }

    #[test]
    fn trusted_associations_invalid_value_returns_error() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
trusted_associations = ["OWNER", "INVALID_VALUE"]
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let result = parsed.into_config(&overrides);
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), ConfigError::InvalidAssociation(v) if v == "INVALID_VALUE")
        );
    }

    #[test]
    fn trusted_associations_case_insensitive() {
        use crate::domain::author_association::AuthorAssociation;

        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
trusted_associations = ["owner", "member"]
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert!(
            config
                .trusted_associations
                .is_trusted(&AuthorAssociation::Owner)
        );
        assert!(
            config
                .trusted_associations
                .is_trusted(&AuthorAssociation::Member)
        );
    }

    // --- shutdown_timeout_secs 変換テスト ---

    #[test]
    fn shutdown_timeout_secs_unset_defaults_to_300s() {
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
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert_eq!(
            config.shutdown_timeout,
            Some(std::time::Duration::from_secs(300)),
            "未設定時はデフォルト 300 秒"
        );
    }

    #[test]
    fn shutdown_timeout_secs_zero_means_infinite_wait() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
shutdown_timeout_secs = 0
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert_eq!(config.shutdown_timeout, None, "0 は無限待機 (None)");
    }

    #[test]
    fn shutdown_timeout_secs_positive_value_used() {
        let toml_str = r#"
owner = "user"
repo = "repo"
default_branch = "main"
shutdown_timeout_secs = 120
"#;
        let parsed: CupolaToml = toml::from_str(toml_str).expect("should parse");
        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = parsed.into_config(&overrides).expect("should succeed");
        assert_eq!(
            config.shutdown_timeout,
            Some(std::time::Duration::from_secs(120)),
            "正の整数は指定秒数タイムアウト"
        );
    }
}
