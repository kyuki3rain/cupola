use std::path::Path;

use crate::application::port::config_loader::{ConfigLoadError, ConfigLoader, DoctorConfigSummary};
use crate::bootstrap::config_loader::load_toml;

pub struct TomlConfigLoader;

impl ConfigLoader for TomlConfigLoader {
    fn load(&self, path: &Path) -> Result<DoctorConfigSummary, ConfigLoadError> {
        let toml = load_toml(path).map_err(|e| {
            use crate::bootstrap::config_loader::ConfigError;
            match e {
                ConfigError::ReadFailed { path, .. } => ConfigLoadError::NotFound { path },
                ConfigError::ParseFailed { path, source } => ConfigLoadError::ParseFailed {
                    path,
                    reason: source.to_string(),
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
