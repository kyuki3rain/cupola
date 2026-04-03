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
}
