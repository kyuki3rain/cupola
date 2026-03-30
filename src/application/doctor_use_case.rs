use std::path::Path;

use crate::application::port::command_runner::CommandRunner;
use crate::bootstrap::config_loader::load_toml;
use crate::domain::check_result::CheckResult;

pub struct DoctorUseCase {
    command_runner: Box<dyn CommandRunner>,
}

impl DoctorUseCase {
    pub fn new(command_runner: Box<dyn CommandRunner>) -> Self {
        Self { command_runner }
    }

    /// 全チェック項目を定められた順序で実行し、結果リストを返す
    /// 順序: git → claude → gh → agent:ready label → model:* labels → cupola.toml → steering → cupola.db
    pub fn run(&self) -> Vec<CheckResult> {
        let mut results = Vec::new();

        // 1. git チェック
        results.push(self.check_git());

        // 2. claude チェック
        results.push(self.check_claude());

        // 3. gh auth チェック
        let gh_result = self.check_gh_auth();
        let gh_passed = !gh_result.is_failed();
        results.push(gh_result);

        // 4. agent:ready ラベルチェック（gh auth 成功時のみ）
        if gh_passed {
            results.push(self.check_agent_ready_label());
        } else {
            results.push(CheckResult::skipped("agent:ready label"));
        }

        // 5. model:* ラベルチェック（gh auth 成功時のみ）
        if gh_passed {
            results.push(self.check_model_labels());
        } else {
            results.push(CheckResult::skipped("model:* labels"));
        }

        // 6. cupola.toml チェック
        results.push(self.check_cupola_toml());

        // 7. steering ディレクトリチェック
        results.push(self.check_steering());

        // 8. cupola.db チェック
        results.push(self.check_cupola_db());

        results
    }

    fn check_git(&self) -> CheckResult {
        let output = self
            .command_runner
            .run("git", &["--version"])
            .unwrap_or_else(
                |_| crate::application::port::command_runner::CommandOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: String::new(),
                },
            );
        if output.success {
            CheckResult::pass("git")
        } else {
            CheckResult::fail(
                "git",
                "git をインストールしてください: https://git-scm.com/",
            )
        }
    }

    fn check_claude(&self) -> CheckResult {
        let output = self
            .command_runner
            .run("claude", &["--version"])
            .unwrap_or_else(
                |_| crate::application::port::command_runner::CommandOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: String::new(),
                },
            );
        if output.success {
            CheckResult::pass("claude")
        } else {
            CheckResult::fail(
                "claude",
                "Claude Code CLI をインストールしてください: npm install -g @anthropic-ai/claude-code",
            )
        }
    }

    fn check_gh_auth(&self) -> CheckResult {
        let output = self
            .command_runner
            .run("gh", &["auth", "status"])
            .unwrap_or_else(
                |_| crate::application::port::command_runner::CommandOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: String::new(),
                },
            );
        if output.success {
            CheckResult::pass("gh auth")
        } else {
            CheckResult::fail("gh auth", "gh auth login を実行して認証してください")
        }
    }

    fn check_agent_ready_label(&self) -> CheckResult {
        let output = self
            .command_runner
            .run("gh", &["label", "list", "--json", "name"])
            .unwrap_or_else(
                |_| crate::application::port::command_runner::CommandOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: String::new(),
                },
            );
        if output.success && output.stdout.contains("agent:ready") {
            CheckResult::pass("agent:ready label")
        } else {
            CheckResult::fail(
                "agent:ready label",
                "gh label create agent:ready を実行してラベルを作成してください",
            )
        }
    }

    fn check_model_labels(&self) -> CheckResult {
        let output = self
            .command_runner
            .run("gh", &["label", "list", "--json", "name"])
            .unwrap_or_else(
                |_| crate::application::port::command_runner::CommandOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: String::new(),
                },
            );
        if output.success && output.stdout.contains("\"model:") {
            CheckResult::pass("model:* labels")
        } else {
            CheckResult::fail(
                "model:* labels",
                "gh label create model:opus && gh label create model:haiku && gh label create model:sonnet を実行してラベルを作成してください",
            )
        }
    }

    fn check_cupola_toml(&self) -> CheckResult {
        let path = Path::new(".cupola/cupola.toml");
        match load_toml(path) {
            Ok(_) => CheckResult::pass("cupola.toml"),
            Err(_) => CheckResult::fail(
                "cupola.toml",
                ".cupola/cupola.toml を作成し、owner, repo, default_branch を設定してください",
            ),
        }
    }

    fn check_steering(&self) -> CheckResult {
        let steering_dir = Path::new(".cupola/steering");
        let has_files = std::fs::read_dir(steering_dir)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
        if has_files {
            CheckResult::pass("steering")
        } else {
            CheckResult::fail(
                "steering",
                ".cupola/steering/ にステアリングファイルを配置してください",
            )
        }
    }

    fn check_cupola_db(&self) -> CheckResult {
        if Path::new(".cupola/cupola.db").exists() {
            CheckResult::pass("cupola.db")
        } else {
            CheckResult::fail(
                "cupola.db",
                "cupola init を実行してデータベースを初期化してください",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::command_runner::test_support::MockCommandRunner;

    fn all_success_runner() -> MockCommandRunner {
        MockCommandRunner::new()
            .with_success("git", &["--version"], "git version 2.40.0")
            .with_success("claude", &["--version"], "1.0.0")
            .with_success("gh", &["auth", "status"], "Logged in")
            .with_success(
                "gh",
                &["label", "list", "--json", "name"],
                r#"[{"name":"agent:ready"},{"name":"model:opus"},{"name":"model:haiku"},{"name":"model:sonnet"}]"#,
            )
    }

    #[test]
    fn all_external_tools_pass() {
        let runner = all_success_runner();
        let use_case = DoctorUseCase::new(Box::new(runner));

        // cupola.toml, steering, db のチェックは失敗するが、外部ツールは成功
        let results = use_case.run();
        assert_eq!(results.len(), 8);

        assert_eq!(results[0].name, "git");
        assert!(!results[0].is_failed());

        assert_eq!(results[1].name, "claude");
        assert!(!results[1].is_failed());

        assert_eq!(results[2].name, "gh auth");
        assert!(!results[2].is_failed());

        assert_eq!(results[3].name, "agent:ready label");
        assert!(!results[3].is_failed());

        assert_eq!(results[4].name, "model:* labels");
        assert!(!results[4].is_failed());
    }

    #[test]
    fn git_failure_sets_remedy() {
        let runner = MockCommandRunner::new()
            .with_failure("git", &["--version"])
            .with_success("claude", &["--version"], "1.0.0")
            .with_success("gh", &["auth", "status"], "Logged in")
            .with_success(
                "gh",
                &["label", "list", "--json", "name"],
                r#"[{"name":"agent:ready"},{"name":"model:opus"}]"#,
            );
        let use_case = DoctorUseCase::new(Box::new(runner));
        let results = use_case.run();

        assert!(results[0].is_failed());
        assert_eq!(results[0].name, "git");
        assert!(results[0].remedy.is_some());
        assert!(results[0].remedy.as_deref().unwrap().contains("git-scm"));
    }

    #[test]
    fn claude_failure_sets_remedy() {
        let runner = MockCommandRunner::new()
            .with_success("git", &["--version"], "git version 2.40.0")
            .with_failure("claude", &["--version"])
            .with_success("gh", &["auth", "status"], "Logged in")
            .with_success(
                "gh",
                &["label", "list", "--json", "name"],
                r#"[{"name":"agent:ready"},{"name":"model:opus"}]"#,
            );
        let use_case = DoctorUseCase::new(Box::new(runner));
        let results = use_case.run();

        assert!(results[1].is_failed());
        assert_eq!(results[1].name, "claude");
        assert!(
            results[1]
                .remedy
                .as_deref()
                .unwrap()
                .contains("npm install")
        );
    }

    #[test]
    fn gh_auth_failure_skips_label_check() {
        let runner = MockCommandRunner::new()
            .with_success("git", &["--version"], "git version 2.40.0")
            .with_success("claude", &["--version"], "1.0.0")
            .with_failure("gh", &["auth", "status"]);
        let use_case = DoctorUseCase::new(Box::new(runner));
        let results = use_case.run();

        assert!(results[2].is_failed()); // gh auth は失敗
        assert_eq!(results[3].name, "agent:ready label");
        assert_eq!(
            results[3].status,
            crate::domain::check_result::CheckStatus::Skipped
        );
        assert!(!results[3].is_failed()); // スキップは失敗としてカウントしない
        assert_eq!(results[4].name, "model:* labels");
        assert_eq!(
            results[4].status,
            crate::domain::check_result::CheckStatus::Skipped
        );
        assert!(!results[4].is_failed()); // スキップは失敗としてカウントしない
    }

    #[test]
    fn label_not_found_sets_failure() {
        let runner = MockCommandRunner::new()
            .with_success("git", &["--version"], "git version 2.40.0")
            .with_success("claude", &["--version"], "1.0.0")
            .with_success("gh", &["auth", "status"], "Logged in")
            .with_success("gh", &["label", "list", "--json", "name"], r#"[]"#);
        let use_case = DoctorUseCase::new(Box::new(runner));
        let results = use_case.run();

        assert!(results[3].is_failed());
        assert!(
            results[3]
                .remedy
                .as_deref()
                .unwrap()
                .contains("gh label create")
        );
    }

    #[test]
    fn model_label_not_found_sets_failure() {
        let runner = MockCommandRunner::new()
            .with_success("git", &["--version"], "git version 2.40.0")
            .with_success("claude", &["--version"], "1.0.0")
            .with_success("gh", &["auth", "status"], "Logged in")
            .with_success(
                "gh",
                &["label", "list", "--json", "name"],
                r#"[{"name":"agent:ready"}]"#,
            );
        let use_case = DoctorUseCase::new(Box::new(runner));
        let results = use_case.run();

        assert_eq!(results[4].name, "model:* labels");
        assert!(results[4].is_failed());
        assert!(
            results[4]
                .remedy
                .as_deref()
                .unwrap()
                .contains("gh label create model:")
        );
    }

    #[test]
    fn run_returns_eight_results() {
        let runner = all_success_runner();
        let use_case = DoctorUseCase::new(Box::new(runner));
        let results = use_case.run();
        assert_eq!(results.len(), 8);
    }

    #[test]
    fn check_order_is_fixed() {
        let runner = all_success_runner();
        let use_case = DoctorUseCase::new(Box::new(runner));
        let results = use_case.run();

        assert_eq!(results[0].name, "git");
        assert_eq!(results[1].name, "claude");
        assert_eq!(results[2].name, "gh auth");
        assert_eq!(results[3].name, "agent:ready label");
        assert_eq!(results[4].name, "model:* labels");
        assert_eq!(results[5].name, "cupola.toml");
        assert_eq!(results[6].name, "steering");
        assert_eq!(results[7].name, "cupola.db");
    }
}
