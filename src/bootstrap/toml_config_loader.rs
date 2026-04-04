use std::path::Path;

use crate::application::port::config_loader::{ConfigLoadError, ConfigLoader, DoctorConfigSummary};
use crate::bootstrap::config_loader::load_toml;

pub struct TomlConfigLoader;

impl ConfigLoader for TomlConfigLoader {
    fn load(&self, path: &Path) -> Result<DoctorConfigSummary, ConfigLoadError> {
        let path_str = path.display().to_string();
        let toml = load_toml(path).map_err(|e| {
            use crate::bootstrap::config_loader::ConfigError;
            match e {
                ConfigError::ReadFailed {
                    path: p,
                    source,
                } => {
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
                    path: path_str,
                    reason: format!("invalid author association value: {value}"),
                },
            }
        })?;

        if toml.default_branch.is_empty() {
            return Err(ConfigLoadError::MissingField {
                field: "default_branch".to_string(),
            });
        }

        Ok(DoctorConfigSummary {
            owner: toml.owner,
            repo: toml.repo,
            default_branch: toml.default_branch,
        })
    }
}
