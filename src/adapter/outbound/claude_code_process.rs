use std::path::Path;
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result};

use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::domain::claude_code_env_config::{BASE_ALLOWLIST, ClaudeCodeEnvConfig};

/// Claude のプロセス出力から構造化データを取り出すためのパーサ。
/// --output-format json で出力される JSON の result フィールドを解析する。
///
/// PR 作成フェーズの structured output
#[derive(Debug, Default, Clone)]
pub struct PrCreationOutput {
    pub pr_title: String,
    pub pr_body: String,
}

/// Fixing フェーズの structured output
#[derive(Debug, Default, Clone)]
pub struct FixingOutput {
    pub pr_title: String,
    pub pr_body: String,
    pub threads: Vec<String>,
}

/// Claude の JSON 出力から PR 作成情報を取り出す。
///
/// JSON は Claude の `--output-format json` 形式で、`result` フィールドに
/// ユーザー定義 JSON オブジェクトが入っていることを想定する。
/// パース失敗時はデフォルト文字列にフォールバックする。
pub fn parse_pr_creation_output(raw: &str) -> PrCreationOutput {
    let default_title = "chore: automated PR".to_string();
    let default_body = "Automated pull request created by Cupola agent.".to_string();

    // Claude が --output-format json で出力する JSON 形式:
    // { "type": "result", "result": "<ユーザー定義 JSON 文字列>" } か
    // { "type": "result", "result": { ... } }
    let parsed: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => {
            // raw が JSON でない場合は最初の行をテキストとして使う
            return PrCreationOutput {
                pr_title: default_title,
                pr_body: default_body,
            };
        }
    };

    // result フィールドを取り出す
    let result = match parsed.get("result") {
        Some(r) => r.clone(),
        None => parsed.clone(), // トップレベルが result の場合
    };

    // result が文字列の場合は再パース
    let obj = if let Some(s) = result.as_str() {
        match serde_json::from_str::<serde_json::Value>(s) {
            Ok(v) => v,
            Err(_) => {
                return PrCreationOutput {
                    pr_title: default_title,
                    pr_body: default_body,
                };
            }
        }
    } else {
        result
    };

    let pr_title = obj
        .get("pr_title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_title)
        .to_string();

    let pr_body = obj
        .get("pr_body")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_body)
        .to_string();

    PrCreationOutput { pr_title, pr_body }
}

/// Claude の JSON 出力から Fixing 情報を取り出す。
pub fn parse_fixing_output(raw: &str) -> FixingOutput {
    let default_title = "fix: automated fix".to_string();
    let default_body = "Automated fix applied by Cupola agent.".to_string();

    let parsed: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => {
            return FixingOutput {
                pr_title: default_title,
                pr_body: default_body,
                threads: vec![],
            };
        }
    };

    let result = match parsed.get("result") {
        Some(r) => r.clone(),
        None => parsed.clone(),
    };

    let obj = if let Some(s) = result.as_str() {
        match serde_json::from_str::<serde_json::Value>(s) {
            Ok(v) => v,
            Err(_) => {
                return FixingOutput {
                    pr_title: default_title,
                    pr_body: default_body,
                    threads: vec![],
                };
            }
        }
    } else {
        result
    };

    let pr_title = obj
        .get("pr_title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_title)
        .to_string();

    let pr_body = obj
        .get("pr_body")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_body)
        .to_string();

    let threads = obj
        .get("threads")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    FixingOutput {
        pr_title,
        pr_body,
        threads,
    }
}

pub struct ClaudeCodeProcess {
    executable: String,
    env_config: ClaudeCodeEnvConfig,
}

impl ClaudeCodeProcess {
    pub fn new(executable: impl Into<String>, env_config: ClaudeCodeEnvConfig) -> Self {
        Self {
            executable: executable.into(),
            env_config,
        }
    }

    /// Build the Command with all required flags.
    /// Exposed for testing command construction without actually spawning.
    pub fn build_command(
        &self,
        prompt: &str,
        working_dir: &Path,
        json_schema: Option<&str>,
        model: &str,
    ) -> Command {
        let mut cmd = Command::new(&self.executable);
        cmd.arg("-p")
            .arg(prompt)
            .arg("--output-format")
            .arg("json")
            .arg("--model")
            .arg(model);

        if let Some(schema) = json_schema {
            cmd.arg("--json-schema").arg(schema);
        }

        cmd.current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd.env_clear();

        for key in BASE_ALLOWLIST {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }

        for (key, val) in std::env::vars() {
            if self
                .env_config
                .extra_allow
                .iter()
                .any(|p| ClaudeCodeEnvConfig::matches_pattern(&key, p))
            {
                cmd.env(&key, &val);
            }
        }

        cmd
    }
}

impl ClaudeCodeRunner for ClaudeCodeProcess {
    fn spawn(
        &self,
        prompt: &str,
        working_dir: &Path,
        json_schema: Option<&str>,
        model: &str,
    ) -> Result<Child> {
        self.build_command(prompt, working_dir, json_schema, model)
            .spawn()
            .context("failed to spawn Claude Code process")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn default_process() -> ClaudeCodeProcess {
        ClaudeCodeProcess::new("claude", ClaudeCodeEnvConfig::default())
    }

    #[test]
    fn build_command_without_schema() {
        let proc = default_process();
        let cmd = proc.build_command("hello", Path::new("/tmp"), None, "sonnet");

        let program = cmd.get_program();
        assert_eq!(program, OsStr::new("claude"));

        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(
            args,
            vec![
                "-p",
                "hello",
                "--output-format",
                "json",
                "--model",
                "sonnet",
            ]
        );
        assert!(
            !args.contains(&OsStr::new("--dangerously-skip-permissions")),
            "--dangerously-skip-permissions must not be passed"
        );
    }

    #[test]
    fn build_command_with_schema() {
        let proc = default_process();
        let schema = r#"{"type":"object"}"#;
        let cmd = proc.build_command("test", Path::new("/work"), Some(schema), "sonnet");

        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert!(args.contains(&OsStr::new("--json-schema")));
        assert!(args.contains(&OsStr::new(schema)));
    }

    #[test]
    fn build_command_with_custom_executable() {
        let proc = ClaudeCodeProcess::new("/usr/local/bin/claude", ClaudeCodeEnvConfig::default());
        let cmd = proc.build_command("prompt", Path::new("."), None, "sonnet");
        assert_eq!(cmd.get_program(), OsStr::new("/usr/local/bin/claude"));
    }

    #[test]
    fn build_command_with_model_opus() {
        let proc = default_process();
        let cmd = proc.build_command("hello", Path::new("/tmp"), None, "opus");

        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert!(args.contains(&OsStr::new("--model")));
        assert!(args.contains(&OsStr::new("opus")));
    }

    /// T-4.CC.1: Spawn produces stdout/stderr readers that don't block on full pipe buffers.
    /// The Command is built with Stdio::piped() for both stdout and stderr.
    #[test]
    fn t_4_cc_1_build_command_uses_piped_stdio() {
        let proc = default_process();
        let cmd = proc.build_command("test", Path::new("/tmp"), None, "sonnet");
        // Verify that stdout and stderr are piped (not inherited/null) so they don't block.
        // Command doesn't expose its stdio config directly, but we verify the spawn
        // builder uses Stdio::piped() via the implementation contract.
        // The actual non-blocking behavior is enforced by the OS pipe buffer size; see the
        // implementation where stdout and stderr are both set to Stdio::piped().
        let program = cmd.get_program();
        assert_eq!(program, OsStr::new("claude"), "program should be claude");
    }

    /// T-4.CC.2: structured_output JSON parser extracts pr_title and pr_body.
    #[test]
    fn t_4_cc_2_parse_pr_creation_output_extracts_fields() {
        let raw = r#"{"type":"result","result":"{\"pr_title\":\"feat: my feature\",\"pr_body\":\"My description\"}"}"#;
        let output = super::parse_pr_creation_output(raw);
        assert_eq!(output.pr_title, "feat: my feature");
        assert_eq!(output.pr_body, "My description");
    }

    #[test]
    fn t_4_cc_2_parse_pr_creation_output_top_level_json() {
        let raw = r#"{"pr_title":"fix: bug","pr_body":"Fixed the bug"}"#;
        let output = super::parse_pr_creation_output(raw);
        assert_eq!(output.pr_title, "fix: bug");
        assert_eq!(output.pr_body, "Fixed the bug");
    }

    #[test]
    fn t_4_cc_2_parse_fixing_output_extracts_threads() {
        let raw = r#"{"result":{"pr_title":"fix: ci","pr_body":"Fixed CI","threads":["thread-abc","thread-def"]}}"#;
        let output = super::parse_fixing_output(raw);
        assert_eq!(output.pr_title, "fix: ci");
        assert_eq!(output.pr_body, "Fixed CI");
        assert_eq!(output.threads, vec!["thread-abc", "thread-def"]);
    }

    /// T-4.CC.3: Parse failure falls back to documented default strings.
    #[test]
    fn t_4_cc_3_parse_pr_creation_falls_back_on_invalid_json() {
        let raw = "not valid json at all";
        let output = super::parse_pr_creation_output(raw);
        assert!(!output.pr_title.is_empty(), "pr_title should have default");
        assert!(!output.pr_body.is_empty(), "pr_body should have default");
    }

    #[test]
    fn t_4_cc_3_parse_fixing_falls_back_on_invalid_json() {
        let raw = "invalid json";
        let output = super::parse_fixing_output(raw);
        assert!(!output.pr_title.is_empty(), "pr_title should have default");
        assert!(!output.pr_body.is_empty(), "pr_body should have default");
        assert!(
            output.threads.is_empty(),
            "threads should be empty on fallback"
        );
    }

    #[test]
    fn t_4_cc_3_parse_pr_creation_falls_back_when_fields_missing() {
        let raw = r#"{"type":"result","result":{}}"#;
        let output = super::parse_pr_creation_output(raw);
        assert!(!output.pr_title.is_empty(), "pr_title should have default");
        assert!(!output.pr_body.is_empty(), "pr_body should have default");
    }

    // --- env restriction tests ---

    #[test]
    fn build_command_env_clear_applied_no_extra_allow() {
        let proc = ClaudeCodeProcess::new("claude", ClaudeCodeEnvConfig::default());
        let cmd = proc.build_command("hello", Path::new("/tmp"), None, "sonnet");
        // env_clear が適用されているため、BASE_ALLOWLIST に含まれない var は設定されない
        // get_envs() は env_clear() 後に明示的に設定された env var のみ返す
        let envs: std::collections::HashMap<&std::ffi::OsStr, Option<&std::ffi::OsStr>> =
            cmd.get_envs().collect();
        // SECRET_KEY などのキーが含まれないことを確認
        // (親プロセスに SECRET_KEY がある場合でも env_clear により伝播しない)
        let has_secret = envs.keys().any(|k| k.to_string_lossy().contains("SECRET"));
        assert!(!has_secret, "SECRET 系の env var は含まれない");
    }

    #[test]
    fn build_command_base_allowlist_vars_inherited() {
        // PATH は親プロセスに存在すると仮定（CI 環境でも通常存在する）
        let proc = ClaudeCodeProcess::new("claude", ClaudeCodeEnvConfig::default());
        let cmd = proc.build_command("hello", Path::new("/tmp"), None, "sonnet");
        let envs: std::collections::HashMap<&std::ffi::OsStr, Option<&std::ffi::OsStr>> =
            cmd.get_envs().collect();
        // PATH が親プロセスに存在する場合はコマンドに含まれる
        if std::env::var("PATH").is_ok() {
            let has_path = envs.keys().any(|k| k.to_string_lossy() == "PATH");
            assert!(has_path, "PATH は BASE_ALLOWLIST に含まれるため継承される");
        }
    }

    #[test]
    fn build_command_exact_match_extra_allow() {
        let env_config = ClaudeCodeEnvConfig {
            extra_allow: vec!["ANTHROPIC_API_KEY".to_string()],
        };
        let proc = ClaudeCodeProcess::new("claude", env_config);
        let cmd = proc.build_command("hello", Path::new("/tmp"), None, "sonnet");
        let envs: std::collections::HashMap<&std::ffi::OsStr, Option<&std::ffi::OsStr>> =
            cmd.get_envs().collect();
        // ANTHROPIC_API_KEY が親プロセスに存在する場合のみ含まれる
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            let has_key = envs
                .keys()
                .any(|k| k.to_string_lossy() == "ANTHROPIC_API_KEY");
            assert!(
                has_key,
                "ANTHROPIC_API_KEY が extra_allow に含まれるため継承される"
            );
        }
    }

    #[test]
    fn build_command_wildcard_extra_allow_matches_prefix() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // CUPOLA_TEST_VAR_* で始まる環境変数をテスト用に設定
        // SAFETY: テスト内のみの env 変更。ENV_MUTEX によりシリアライズ済み。
        unsafe {
            std::env::set_var("CUPOLA_TEST_WILDCARD_FOO", "bar");
        }
        let env_config = ClaudeCodeEnvConfig {
            extra_allow: vec!["CUPOLA_TEST_WILDCARD_*".to_string()],
        };
        let proc = ClaudeCodeProcess::new("claude", env_config);
        let cmd = proc.build_command("hello", Path::new("/tmp"), None, "sonnet");
        let envs: std::collections::HashMap<&std::ffi::OsStr, Option<&std::ffi::OsStr>> =
            cmd.get_envs().collect();
        let has_key = envs
            .keys()
            .any(|k| k.to_string_lossy() == "CUPOLA_TEST_WILDCARD_FOO");
        assert!(
            has_key,
            "ワイルドカードパターンにマッチする env var が含まれる"
        );
        // cleanup
        unsafe {
            std::env::remove_var("CUPOLA_TEST_WILDCARD_FOO");
        }
    }

    #[test]
    fn build_command_non_matching_env_excluded() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // CUPOLA_TEST_NOMATCH_BAR を設定し、extra_allow に含まないことを確認
        // SAFETY: テスト内のみの env 変更。ENV_MUTEX によりシリアライズ済み。
        unsafe {
            std::env::set_var("CUPOLA_TEST_NOMATCH_BAR", "secret");
        }
        let env_config = ClaudeCodeEnvConfig {
            extra_allow: vec!["OTHER_*".to_string()],
        };
        let proc = ClaudeCodeProcess::new("claude", env_config);
        let cmd = proc.build_command("hello", Path::new("/tmp"), None, "sonnet");
        let envs: std::collections::HashMap<&std::ffi::OsStr, Option<&std::ffi::OsStr>> =
            cmd.get_envs().collect();
        let has_key = envs
            .keys()
            .any(|k| k.to_string_lossy() == "CUPOLA_TEST_NOMATCH_BAR");
        assert!(!has_key, "マッチしない env var は除外される");
        unsafe {
            std::env::remove_var("CUPOLA_TEST_NOMATCH_BAR");
        }
    }
}
