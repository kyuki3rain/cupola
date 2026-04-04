use std::path::Path;

use crate::application::port::config_loader::{ConfigLoadError, ConfigLoader, DoctorConfigSummary};
use crate::bootstrap::config_loader::{CliOverrides, load_toml};

pub struct TomlConfigLoader;

impl ConfigLoader for TomlConfigLoader {
    fn load(&self, path: &Path) -> Result<DoctorConfigSummary, ConfigLoadError> {
        let path_str = path.display().to_string();
        let toml = load_toml(path).map_err(|e| {
            use crate::bootstrap::config_loader::ConfigError;
            match e {
                ConfigError::ReadFailed { path: p, source } => {
                    if source.kind() == std::io::ErrorKind::NotFound {
                        ConfigLoadError::NotFound { path: p }
                    } else {
                        ConfigLoadError::ReadFailed {
                            path: p,
                            reason: source.to_string(),
                        }
                    }
                }
                ConfigError::ParseFailed { path: p, source } => ConfigLoadError::ParseFailed {
                    path: p,
                    reason: source.to_string(),
                },
                // InvalidAssociation は into_config() で発生するため load_toml() では到達しないが、
                // exhaustive matching のために処理する。
                ConfigError::InvalidAssociation(value) => ConfigLoadError::ParseFailed {
                    path: path_str.clone(),
                    reason: format!("invalid author association value: {value}"),
                },
            }
        })?;

        let summary = DoctorConfigSummary {
            owner: toml.owner.clone(),
            repo: toml.repo.clone(),
            default_branch: toml.default_branch.clone(),
        };

        let overrides = CliOverrides {
            polling_interval_secs: None,
            log_level: None,
        };
        let config = toml.into_config(&overrides).map_err(|e| {
            use crate::bootstrap::config_loader::ConfigError;
            match e {
                ConfigError::InvalidAssociation(value) => ConfigLoadError::ParseFailed {
                    path: path_str.clone(),
                    reason: format!("invalid author association value: {value}"),
                },
                _ => ConfigLoadError::ParseFailed {
                    path: path_str.clone(),
                    reason: e.to_string(),
                },
            }
        })?;

        config
            .validate()
            .map_err(|reason| ConfigLoadError::ValidationFailed { reason })?;

        Ok(summary)
    }
}
