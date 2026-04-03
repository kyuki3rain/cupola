use crate::domain::phase::Phase;
use crate::domain::task_weight::TaskWeight;

/// フェーズ別モデル設定
#[derive(Debug, Clone)]
pub struct PerPhaseModels {
    pub design: Option<String>,
    pub design_fix: Option<String>,
    pub implementation: Option<String>,
    pub implementation_fix: Option<String>,
}

/// weight 単位のモデル設定
#[derive(Debug, Clone)]
pub enum WeightModelConfig {
    Uniform(String),
    PerPhase(PerPhaseModels),
}

impl WeightModelConfig {
    /// フェーズに対応するモデル名を返す。設定がなければ None。
    fn resolve_for_phase(&self, phase: Phase) -> Option<&str> {
        match self {
            Self::Uniform(model) => Some(model.as_str()),
            Self::PerPhase(per_phase) => {
                // exact phase
                let exact = match phase {
                    Phase::Design => per_phase.design.as_deref(),
                    Phase::DesignFix => per_phase.design_fix.as_deref(),
                    Phase::Implementation => per_phase.implementation.as_deref(),
                    Phase::ImplementationFix => per_phase.implementation_fix.as_deref(),
                };
                if exact.is_some() {
                    return exact;
                }
                // base phase fallback
                if let Some(base) = phase.base() {
                    match base {
                        Phase::Design => per_phase.design.as_deref(),
                        Phase::Implementation => per_phase.implementation.as_deref(),
                        _ => None,
                    }
                } else {
                    None
                }
            }
        }
    }
}

/// weight × phase → model 名の解決ロジック
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// グローバルデフォルト（`model = "sonnet"` に対応）
    pub default_model: String,
    pub light: Option<WeightModelConfig>,
    pub medium: Option<WeightModelConfig>,
    pub heavy: Option<WeightModelConfig>,
}

impl ModelConfig {
    /// 4 段フォールバックチェーンでモデル名を解決する。
    /// phase が None の場合はグローバルデフォルトを返す。
    pub fn resolve(&self, weight: TaskWeight, phase: Option<Phase>) -> &str {
        let Some(phase) = phase else {
            return &self.default_model;
        };

        let weight_config = match weight {
            TaskWeight::Light => self.light.as_ref(),
            TaskWeight::Medium => self.medium.as_ref(),
            TaskWeight::Heavy => self.heavy.as_ref(),
        };

        let Some(wc) = weight_config else {
            return &self.default_model;
        };

        wc.resolve_for_phase(phase).unwrap_or(&self.default_model)
    }

    /// `model = "sonnet"` 相当のデフォルト設定を生成する。
    pub fn new_default(default_model: String) -> Self {
        Self {
            default_model,
            light: None,
            medium: None,
            heavy: None,
        }
    }

    /// モデル設定の整合性を検証する。空文字列のモデル名は拒否する。
    pub fn validate(&self) -> Result<(), String> {
        if self.default_model.is_empty() {
            return Err("models.default_model must not be empty".to_string());
        }
        for (name, wc) in [
            ("light", &self.light),
            ("medium", &self.medium),
            ("heavy", &self.heavy),
        ] {
            if let Some(wc) = wc {
                wc.validate(name)?;
            }
        }
        Ok(())
    }
}

impl WeightModelConfig {
    fn validate(&self, weight_name: &str) -> Result<(), String> {
        match self {
            Self::Uniform(model) => {
                if model.is_empty() {
                    return Err(format!("models.{weight_name} must not be empty"));
                }
            }
            Self::PerPhase(per_phase) => {
                for (phase_name, val) in [
                    ("design", &per_phase.design),
                    ("design_fix", &per_phase.design_fix),
                    ("implementation", &per_phase.implementation),
                    ("implementation_fix", &per_phase.implementation_fix),
                ] {
                    if let Some(model) = val
                        && model.is_empty()
                    {
                        return Err(format!(
                            "models.{weight_name}.{phase_name} must not be empty"
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform_config(default: &str) -> ModelConfig {
        ModelConfig::new_default(default.to_string())
    }

    fn config_with_heavy_uniform(default: &str, heavy: &str) -> ModelConfig {
        ModelConfig {
            default_model: default.to_string(),
            light: None,
            medium: None,
            heavy: Some(WeightModelConfig::Uniform(heavy.to_string())),
        }
    }

    fn config_with_heavy_per_phase(
        default: &str,
        design: Option<&str>,
        implementation: Option<&str>,
    ) -> ModelConfig {
        ModelConfig {
            default_model: default.to_string(),
            light: None,
            medium: None,
            heavy: Some(WeightModelConfig::PerPhase(PerPhaseModels {
                design: design.map(String::from),
                design_fix: None,
                implementation: implementation.map(String::from),
                implementation_fix: None,
            })),
        }
    }

    #[test]
    fn resolve_phase_none_returns_default() {
        let config = uniform_config("sonnet");
        assert_eq!(config.resolve(TaskWeight::Medium, None), "sonnet");
        assert_eq!(config.resolve(TaskWeight::Heavy, None), "sonnet");
        assert_eq!(config.resolve(TaskWeight::Light, None), "sonnet");
    }

    #[test]
    fn resolve_no_weight_config_returns_default() {
        let config = uniform_config("sonnet");
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::Design)),
            "sonnet"
        );
    }

    #[test]
    fn resolve_uniform_weight_config() {
        let config = config_with_heavy_uniform("sonnet", "opus");
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::Design)),
            "opus"
        );
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::Implementation)),
            "opus"
        );
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::DesignFix)),
            "opus"
        );
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::ImplementationFix)),
            "opus"
        );
    }

    #[test]
    fn resolve_medium_falls_back_to_default_when_no_medium_config() {
        let config = config_with_heavy_uniform("sonnet", "opus");
        assert_eq!(
            config.resolve(TaskWeight::Medium, Some(Phase::Design)),
            "sonnet"
        );
    }

    #[test]
    fn resolve_per_phase_exact_match() {
        let config = config_with_heavy_per_phase("sonnet", Some("opus"), Some("haiku"));
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::Design)),
            "opus"
        );
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::Implementation)),
            "haiku"
        );
    }

    #[test]
    fn resolve_per_phase_fallback_to_base() {
        let config = config_with_heavy_per_phase("sonnet", Some("opus"), Some("haiku"));
        // DesignFix → DesignFix is None → fallback to Design (opus)
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::DesignFix)),
            "opus"
        );
        // ImplementationFix → ImplementationFix is None → fallback to Implementation (haiku)
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::ImplementationFix)),
            "haiku"
        );
    }

    #[test]
    fn resolve_per_phase_fallback_to_global_default() {
        // Neither design nor implementation is set
        let config = ModelConfig {
            default_model: "sonnet".to_string(),
            light: None,
            medium: None,
            heavy: Some(WeightModelConfig::PerPhase(PerPhaseModels {
                design: None,
                design_fix: None,
                implementation: None,
                implementation_fix: None,
            })),
        };
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::Design)),
            "sonnet"
        );
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::DesignFix)),
            "sonnet"
        );
    }

    #[test]
    fn resolve_per_phase_explicit_fix_overrides_base() {
        let config = ModelConfig {
            default_model: "sonnet".to_string(),
            light: None,
            medium: None,
            heavy: Some(WeightModelConfig::PerPhase(PerPhaseModels {
                design: Some("opus".to_string()),
                design_fix: Some("haiku".to_string()),
                implementation: None,
                implementation_fix: None,
            })),
        };
        // Explicit design_fix should win over base(design)
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::DesignFix)),
            "haiku"
        );
        // Design still returns opus
        assert_eq!(
            config.resolve(TaskWeight::Heavy, Some(Phase::Design)),
            "opus"
        );
    }

    #[test]
    fn validate_accepts_valid_config() {
        let config = config_with_heavy_uniform("sonnet", "opus");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_default_model() {
        let config = uniform_config("");
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_uniform_weight_model() {
        let config = config_with_heavy_uniform("sonnet", "");
        let err = config.validate().unwrap_err();
        assert!(err.contains("heavy"), "error should mention field: {err}");
    }

    #[test]
    fn validate_rejects_empty_per_phase_model() {
        let config = ModelConfig {
            default_model: "sonnet".to_string(),
            light: None,
            medium: None,
            heavy: Some(WeightModelConfig::PerPhase(PerPhaseModels {
                design: Some("".to_string()),
                design_fix: None,
                implementation: None,
                implementation_fix: None,
            })),
        };
        let err = config.validate().unwrap_err();
        assert!(
            err.contains("heavy") && err.contains("design"),
            "error should mention field: {err}"
        );
    }

    #[test]
    fn validate_accepts_per_phase_with_none_fields() {
        let config = config_with_heavy_per_phase("sonnet", Some("opus"), None);
        assert!(config.validate().is_ok());
    }
}
