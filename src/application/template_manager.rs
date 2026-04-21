use crate::domain::claude_settings::{ClaudePermissions, ClaudeSettings};

/// コンパイル時埋め込みテンプレート。(key, json_content) のペア。
const TEMPLATES: &[(&str, &str)] = &[
    (
        "base",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/claude-settings/base.json"
        )),
    ),
    (
        "rust",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/claude-settings/rust.json"
        )),
    ),
    (
        "typescript",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/claude-settings/typescript.json"
        )),
    ),
    (
        "python",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/claude-settings/python.json"
        )),
    ),
    (
        "go",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/claude-settings/go.json"
        )),
    ),
];

#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("unknown template key '{key}'. Available: {available}")]
    UnknownTemplate { key: String, available: String },
    #[error("failed to parse template '{key}': {source}")]
    ParseError {
        key: String,
        source: serde_json::Error,
    },
}

pub struct TemplateManager;

impl TemplateManager {
    /// 指定キーのテンプレートをロードして union マージした `ClaudeSettings` を返す。
    ///
    /// - `base` は常に先頭に 1 回だけ適用される（明示的に渡されても二重適用しない）。
    /// - 未知キーは `TemplateError::UnknownTemplate` を返す。
    /// - 後提条件: `allow`/`deny` に重複なし。
    pub fn build_settings(templates: &[&str]) -> Result<ClaudeSettings, TemplateError> {
        // 未知キーチェック（base を含む全キーに対して）
        for key in templates {
            Self::find_template(key)?;
        }

        // base を先頭に 1 回だけ適用し、残りを重複排除して順序保持で結合
        let mut keys: Vec<&str> = Vec::with_capacity(templates.len() + 1);
        keys.push("base");
        for key in templates {
            if *key != "base" && !keys.contains(key) {
                keys.push(key);
            }
        }

        let mut merged_allow: Vec<String> = Vec::new();
        let mut merged_deny: Vec<String> = Vec::new();

        for key in keys {
            let settings = Self::load(key)?;
            for entry in settings.permissions.allow {
                if !merged_allow.contains(&entry) {
                    merged_allow.push(entry);
                }
            }
            for entry in settings.permissions.deny {
                if !merged_deny.contains(&entry) {
                    merged_deny.push(entry);
                }
            }
        }

        Ok(ClaudeSettings {
            permissions: ClaudePermissions {
                allow: merged_allow,
                deny: merged_deny,
            },
        })
    }

    /// 利用可能なテンプレートキー一覧を返す。
    pub fn list_available() -> &'static [&'static str] {
        static AVAILABLE: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
        AVAILABLE
            .get_or_init(|| TEMPLATES.iter().map(|(key, _)| *key).collect())
            .as_slice()
    }

    fn find_template(key: &str) -> Result<(), TemplateError> {
        if TEMPLATES.iter().any(|(k, _)| *k == key) {
            Ok(())
        } else {
            Err(TemplateError::UnknownTemplate {
                key: key.to_string(),
                available: Self::list_available().join(", "),
            })
        }
    }

    fn load(key: &str) -> Result<ClaudeSettings, TemplateError> {
        let content = TEMPLATES
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, c)| *c)
            .ok_or_else(|| TemplateError::UnknownTemplate {
                key: key.to_string(),
                available: Self::list_available().join(", "),
            })?;

        serde_json::from_str(content).map_err(|e| TemplateError::ParseError {
            key: key.to_string(),
            source: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_settings_base_only() {
        let settings = TemplateManager::build_settings(&[]).expect("build");
        assert!(!settings.permissions.allow.is_empty(), "base should have allow entries");
        assert!(!settings.permissions.deny.is_empty(), "base should have deny entries");
    }

    #[test]
    fn build_settings_with_rust_template() {
        let settings = TemplateManager::build_settings(&["rust"]).expect("build");
        assert!(
            settings.permissions.allow.iter().any(|a| a.contains("cargo")),
            "rust template should add cargo commands"
        );
    }

    #[test]
    fn build_settings_with_multiple_templates() {
        let settings = TemplateManager::build_settings(&["rust", "typescript"]).expect("build");
        assert!(
            settings.permissions.allow.iter().any(|a| a.contains("cargo")),
            "rust template should be present"
        );
        assert!(
            settings.permissions.allow.iter().any(|a| a.contains("npm")),
            "typescript template should be present"
        );
    }

    #[test]
    fn build_settings_unknown_key_returns_error() {
        let err = TemplateManager::build_settings(&["unknown-key"]).expect_err("should fail");
        match err {
            TemplateError::UnknownTemplate { key, available } => {
                assert_eq!(key, "unknown-key");
                assert!(available.contains("rust"), "available should list rust");
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn build_settings_base_duplicate_not_doubled() {
        let settings_explicit = TemplateManager::build_settings(&["base"]).expect("build");
        let settings_implicit = TemplateManager::build_settings(&[]).expect("build");
        assert_eq!(
            settings_explicit.permissions.allow,
            settings_implicit.permissions.allow,
            "explicit 'base' should not be applied twice"
        );
    }

    #[test]
    fn build_settings_empty_slice_returns_base() {
        let settings = TemplateManager::build_settings(&[]).expect("build");
        assert!(!settings.permissions.allow.is_empty());
    }

    #[test]
    fn build_settings_no_duplicates_in_allow() {
        let settings = TemplateManager::build_settings(&["rust"]).expect("build");
        let allow = &settings.permissions.allow;
        let mut deduped = allow.clone();
        deduped.dedup();
        // dedup only removes consecutive duplicates so use a HashSet check
        let unique_count = allow.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(allow.len(), unique_count, "allow should have no duplicates");
    }

    #[test]
    fn list_available_contains_expected_keys() {
        let keys = TemplateManager::list_available();
        assert!(keys.contains(&"base"));
        assert!(keys.contains(&"rust"));
        assert!(keys.contains(&"typescript"));
        assert!(keys.contains(&"python"));
        assert!(keys.contains(&"go"));
    }
}
