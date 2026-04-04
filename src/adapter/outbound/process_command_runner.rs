use std::path::Path;
use std::{io, process::Command};

use anyhow::Result;

use crate::application::port::command_runner::{CommandOutput, CommandRunner};

/// `std::process::Command` を使用して外部コマンドを実行するアダプター
pub struct ProcessCommandRunner;

impl CommandRunner for ProcessCommandRunner {
    fn run_in_dir(&self, program: &str, args: &[&str], dir: &Path) -> Result<CommandOutput> {
        if !dir.is_dir() {
            return Ok(CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: format!("invalid working directory: {}", dir.display()),
            });
        }

        match Command::new(program).args(args).current_dir(dir).output() {
            Ok(output) => Ok(CommandOutput {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }),
            Err(err) => Ok(map_spawn_error(program, err)),
        }
    }
}

fn map_spawn_error(program: &str, err: io::Error) -> CommandOutput {
    let stderr = match err.kind() {
        io::ErrorKind::NotFound => format!("{program}: command not found"),
        _ => err.to_string(),
    };

    CommandOutput {
        success: false,
        stdout: String::new(),
        stderr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_existing_command_succeeds() {
        let runner = ProcessCommandRunner;
        // `true` コマンドは常に exit 0 で終了する
        #[cfg(unix)]
        {
            let output = runner.run("true", &[]).expect("run should not error");
            assert!(output.success);
        }
    }

    #[test]
    fn run_git_version_succeeds() {
        let runner = ProcessCommandRunner;
        let output = runner
            .run("git", &["--version"])
            .expect("run should not error");
        // git がインストールされていれば成功
        assert!(output.success);
        assert!(output.stdout.contains("git"));
    }

    #[test]
    fn run_nonexistent_command_returns_failure_not_panic() {
        let runner = ProcessCommandRunner;
        let output = runner
            .run("__cupola_nonexistent_command__", &[])
            .expect("run should not error even for missing commands");
        assert!(!output.success);
        assert!(output.stderr.contains("command not found"));
    }

    #[test]
    fn run_in_dir_reports_invalid_working_directory() {
        let runner = ProcessCommandRunner;
        let output = runner
            .run_in_dir(
                "git",
                &["--version"],
                Path::new("/definitely/missing/cupola-dir"),
            )
            .expect("run should not error even for invalid dir");
        assert!(!output.success);
        assert!(output.stderr.contains("invalid working directory"));
    }
}
