/// Claude Code サブプロセスに適用する permission 設定。
///
/// `cupola.toml` の `[claude_code.permissions]` セクションから読み込まれ、
/// `ClaudeCodeRunner::spawn` 時に `--allowedTools` / `--disallowedTools` の CLI フラグに
/// 変換されて渡される。`.claude/settings.json` には書き込まれないため、対話 claude の
/// permission 設定と完全に分離される。
///
/// # フィールド
///
/// - `templates`: `TemplateManager` が解決するテンプレートキー一覧 (例: `["rust"]`、
///   `["rust", "devbox"]`)。空の場合は `base` のみが暗黙に適用される
/// - `extra_allow`: テンプレート解決結果の `allow` リストに追加するエントリ
/// - `extra_deny`: テンプレート解決結果の `deny` リストに追加するエントリ
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaudeCodePermissionsConfig {
    pub templates: Vec<String>,
    pub extra_allow: Vec<String>,
    pub extra_deny: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let cfg = ClaudeCodePermissionsConfig::default();
        assert!(cfg.templates.is_empty());
        assert!(cfg.extra_allow.is_empty());
        assert!(cfg.extra_deny.is_empty());
    }
}
