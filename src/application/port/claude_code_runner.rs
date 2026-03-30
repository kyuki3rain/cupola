use anyhow::Result;
use std::path::Path;
use std::process::Child;

pub trait ClaudeCodeRunner: Send + Sync {
    fn spawn(
        &self,
        prompt: &str,
        working_dir: &Path,
        json_schema: Option<&str>,
        model: &str,
    ) -> Result<Child>;
}
