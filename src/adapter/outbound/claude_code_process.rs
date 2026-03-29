use std::path::Path;
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result};

use crate::application::port::claude_code_runner::ClaudeCodeRunner;

pub struct ClaudeCodeProcess {
    executable: String,
}

impl ClaudeCodeProcess {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    /// Build the Command with all required flags.
    /// Exposed for testing command construction without actually spawning.
    pub fn build_command(
        &self,
        prompt: &str,
        working_dir: &Path,
        json_schema: Option<&str>,
    ) -> Command {
        let mut cmd = Command::new(&self.executable);
        cmd.arg("-p")
            .arg(prompt)
            .arg("--output-format")
            .arg("json")
            .arg("--dangerously-skip-permissions");

        if let Some(schema) = json_schema {
            cmd.arg("--json-schema").arg(schema);
        }

        cmd.current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd
    }
}

impl ClaudeCodeRunner for ClaudeCodeProcess {
    fn spawn(&self, prompt: &str, working_dir: &Path, json_schema: Option<&str>) -> Result<Child> {
        self.build_command(prompt, working_dir, json_schema)
            .spawn()
            .context("failed to spawn Claude Code process")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn build_command_without_schema() {
        let proc = ClaudeCodeProcess::new("claude");
        let cmd = proc.build_command("hello", Path::new("/tmp"), None);

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
                "--dangerously-skip-permissions",
            ]
        );
    }

    #[test]
    fn build_command_with_schema() {
        let proc = ClaudeCodeProcess::new("claude");
        let schema = r#"{"type":"object"}"#;
        let cmd = proc.build_command("test", Path::new("/work"), Some(schema));

        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert!(args.contains(&OsStr::new("--json-schema")));
        assert!(args.contains(&OsStr::new(schema)));
    }

    #[test]
    fn build_command_with_custom_executable() {
        let proc = ClaudeCodeProcess::new("/usr/local/bin/claude");
        let cmd = proc.build_command("prompt", Path::new("."), None);
        assert_eq!(cmd.get_program(), OsStr::new("/usr/local/bin/claude"));
    }
}
