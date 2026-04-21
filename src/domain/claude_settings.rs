/// `.claude/settings.json` の `permissions` セクションを表す値オブジェクト。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClaudeSettings {
    pub permissions: ClaudePermissions,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ClaudePermissions {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

impl ClaudeSettings {
    pub fn new(allow: Vec<String>, deny: Vec<String>) -> Self {
        Self {
            permissions: ClaudePermissions { allow, deny },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_deserialize_roundtrip() {
        let settings = ClaudeSettings::new(
            vec!["Bash(git status)".to_string()],
            vec!["Bash(rm -rf*)".to_string()],
        );
        let json = serde_json::to_string(&settings).expect("serialize");
        let parsed: ClaudeSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.permissions.allow, settings.permissions.allow);
        assert_eq!(parsed.permissions.deny, settings.permissions.deny);
    }

    #[test]
    fn deserialize_with_missing_fields_uses_empty_vecs() {
        let json = r#"{"permissions": {}}"#;
        let parsed: ClaudeSettings = serde_json::from_str(json).expect("deserialize");
        assert!(parsed.permissions.allow.is_empty());
        assert!(parsed.permissions.deny.is_empty());
    }

    #[test]
    fn deserialize_with_only_allow() {
        let json = r#"{"permissions": {"allow": ["Bash(cargo build*)"] }}"#;
        let parsed: ClaudeSettings = serde_json::from_str(json).expect("deserialize");
        assert_eq!(parsed.permissions.allow, vec!["Bash(cargo build*)"]);
        assert!(parsed.permissions.deny.is_empty());
    }
}
