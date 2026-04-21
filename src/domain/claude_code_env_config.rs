pub const BASE_ALLOWLIST: &[&str] = &["HOME", "PATH", "USER", "LANG", "LC_ALL", "TERM"];

#[derive(Debug, Clone, Default)]
pub struct ClaudeCodeEnvConfig {
    pub extra_allow: Vec<String>,
}

impl ClaudeCodeEnvConfig {
    /// キーがパターンにマッチするか判定する。
    /// パターンが `*` で終わる場合はプレフィックス一致、それ以外は完全一致。
    /// 先頭 `*` や中間 `*` はリテラルとして扱われる。
    pub fn matches_pattern(key: &str, pattern: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix('*') {
            key.starts_with(prefix)
        } else {
            key == pattern
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_pattern_exact_match() {
        assert!(ClaudeCodeEnvConfig::matches_pattern("HOME", "HOME"));
        assert!(ClaudeCodeEnvConfig::matches_pattern(
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_API_KEY"
        ));
    }

    #[test]
    fn matches_pattern_exact_no_match() {
        assert!(!ClaudeCodeEnvConfig::matches_pattern("HOME2", "HOME"));
        assert!(!ClaudeCodeEnvConfig::matches_pattern(
            "ANTHROPIC",
            "ANTHROPIC_API_KEY"
        ));
    }

    #[test]
    fn matches_pattern_suffix_wildcard_match() {
        assert!(ClaudeCodeEnvConfig::matches_pattern(
            "CLAUDE_FOO",
            "CLAUDE_*"
        ));
        assert!(ClaudeCodeEnvConfig::matches_pattern(
            "CLAUDE_BAR",
            "CLAUDE_*"
        ));
        assert!(ClaudeCodeEnvConfig::matches_pattern(
            "AWS_SECRET_ACCESS_KEY",
            "AWS_*"
        ));
    }

    #[test]
    fn matches_pattern_suffix_wildcard_no_match() {
        assert!(!ClaudeCodeEnvConfig::matches_pattern(
            "OTHER_FOO",
            "CLAUDE_*"
        ));
        assert!(!ClaudeCodeEnvConfig::matches_pattern(
            "MYCLAUD_X",
            "CLAUDE_*"
        ));
    }

    #[test]
    fn matches_pattern_middle_wildcard_treated_as_literal() {
        // 中間の * はリテラル扱い（完全一致のみ）
        assert!(!ClaudeCodeEnvConfig::matches_pattern(
            "MY_FOO_VAR",
            "MY_*_VAR"
        ));
        assert!(ClaudeCodeEnvConfig::matches_pattern("MY_*_VAR", "MY_*_VAR"));
    }

    #[test]
    fn matches_pattern_leading_wildcard_treated_as_literal() {
        // 先頭の * はリテラル扱い
        assert!(!ClaudeCodeEnvConfig::matches_pattern("_API_KEY", "*_KEY"));
        assert!(ClaudeCodeEnvConfig::matches_pattern("*_KEY", "*_KEY"));
    }

    #[test]
    fn default_extra_allow_is_empty() {
        let cfg = ClaudeCodeEnvConfig::default();
        assert!(cfg.extra_allow.is_empty());
    }
}
