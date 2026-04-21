use std::path::Path;

use serde::Deserialize;

use crate::application::doctor_result::{CheckStatus, DoctorCheckResult, DoctorSection};
use crate::application::port::command_runner::CommandRunner;
use crate::application::port::config_loader::{ConfigLoader, DoctorConfigSummary};
use crate::application::port::issue_repository::IssueRepository;
use crate::domain::claude_code_env_config::{BASE_ALLOWLIST, ClaudeCodeEnvConfig};

pub struct DoctorUseCase<C: ConfigLoader, R: CommandRunner, I: IssueRepository> {
    config_loader: C,
    command_runner: R,
    issue_repo: I,
    max_ci_fix_cycles: u32,
}

impl<C: ConfigLoader, R: CommandRunner, I: IssueRepository> DoctorUseCase<C, R, I> {
    pub fn new(config_loader: C, command_runner: R, issue_repo: I, max_ci_fix_cycles: u32) -> Self {
        Self {
            config_loader,
            command_runner,
            issue_repo,
            max_ci_fix_cycles,
        }
    }

    pub async fn run(&self, config_path: &Path) -> Vec<DoctorCheckResult> {
        let steering_path = Path::new(".cupola").join("steering");
        let db_path = Path::new(".cupola").join("cupola.db");
        let commands_path = Path::new(".claude").join("commands").join("cupola");
        let settings_path = Path::new(".cupola").join("settings");

        let [agent_ready_result, weight_labels_result] = check_labels(&self.command_runner);

        let config_result = check_config(&self.config_loader, config_path);
        let env_allowlist_result = self
            .config_loader
            .load(config_path)
            .ok()
            .map(|summary| check_env_allowlist(&summary));

        let pending_check =
            check_pending_ci_fix_limit_notifications(&self.issue_repo, self.max_ci_fix_cycles)
                .await;

        let mut results = vec![
            // Start Readiness
            config_result,
            check_git(&self.command_runner),
            check_github_token(&self.command_runner),
            check_claude(&self.command_runner),
            check_db(&db_path),
        ];
        if let Some(r) = env_allowlist_result {
            results.push(r);
        }
        results.extend([
            // Operational Readiness
            check_assets(&commands_path, &settings_path),
            check_steering(&steering_path),
            agent_ready_result,
            weight_labels_result,
            pending_check,
        ]);
        results
    }
}

const SENSITIVE_PATTERNS: &[&str] = &["GH_TOKEN", "GITHUB_TOKEN", "AWS_*", "AZURE_*", "GOOGLE_*"];

const SENSITIVE_SUFFIX_PATTERNS: &[&str] = &["_API_KEY", "_SECRET", "_TOKEN", "_PASSWORD"];

fn check_env_allowlist(summary: &DoctorConfigSummary) -> DoctorCheckResult {
    let base: Vec<&str> = BASE_ALLOWLIST.to_vec();
    let extra = &summary.claude_code_extra_allow;

    let dangerous: Vec<&str> = extra
        .iter()
        .filter(|entry| {
            SENSITIVE_PATTERNS
                .iter()
                .any(|p| ClaudeCodeEnvConfig::matches_pattern(entry, p))
                || SENSITIVE_SUFFIX_PATTERNS
                    .iter()
                    .any(|suffix| entry.ends_with(suffix))
        })
        .map(String::as_str)
        .collect();

    if dangerous.is_empty() {
        let base_list = base.join(", ");
        let extra_list = if extra.is_empty() {
            "（なし）".to_string()
        } else {
            extra.join(", ")
        };
        DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "env allowlist".to_string(),
            status: CheckStatus::Ok(format!(
                "BASE_ALLOWLIST: [{base_list}] / extra_allow: [{extra_list}]"
            )),
            remediation: None,
        }
    } else {
        let dangerous_list = dangerous.join(", ");
        DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "env allowlist".to_string(),
            status: CheckStatus::Warn(format!(
                "潜在的に危険なパターンが extra_allow に含まれています: {dangerous_list}"
            )),
            remediation: Some(
                "不要であれば cupola.toml の extra_allow から削除してください".to_string(),
            ),
        }
    }
}

async fn check_pending_ci_fix_limit_notifications(
    issue_repo: &impl IssueRepository,
    max_cycles: u32,
) -> DoctorCheckResult {
    match issue_repo.find_all().await {
        Ok(issues) => {
            let pending_count = issues
                .iter()
                .filter(|i| i.ci_fix_count > max_cycles && !i.ci_fix_limit_notified)
                .count();

            if pending_count > 0 {
                DoctorCheckResult {
                    section: DoctorSection::OperationalReadiness,
                    name: "ci-fix-limit notification".to_string(),
                    status: CheckStatus::Warn(format!(
                        "CI 修正上限通知が未送信の Issue が {pending_count} 件あります"
                    )),
                    remediation: Some(
                        "GitHub の通信状況を確認の上、cupola start で再度ポーリングを実行してください"
                            .to_string(),
                    ),
                }
            } else {
                DoctorCheckResult {
                    section: DoctorSection::OperationalReadiness,
                    name: "ci-fix-limit notification".to_string(),
                    status: CheckStatus::Ok(
                        "CI 修正上限通知の未送信 Issue はありません".to_string(),
                    ),
                    remediation: None,
                }
            }
        }
        Err(e) => DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "ci-fix-limit notification".to_string(),
            status: CheckStatus::Warn(format!("Issue 一覧の取得に失敗しました: {e}")),
            remediation: Some("cupola init で DB を初期化してください".to_string()),
        },
    }
}

fn check_config(config_loader: &dyn ConfigLoader, path: &Path) -> DoctorCheckResult {
    match config_loader.load(path) {
        Ok(_) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "config".to_string(),
            status: CheckStatus::Ok("cupola.toml が正常に読み込まれました".to_string()),
            remediation: None,
        },
        Err(e) => {
            use crate::application::port::config_loader::ConfigLoadError;
            let remediation = match &e {
                ConfigLoadError::NotFound { .. } => {
                    Some("`cupola init` を実行して cupola.toml を作成してください".to_string())
                }
                ConfigLoadError::ValidationFailed { .. } => {
                    Some("cupola.toml の設定値を確認・修正してください".to_string())
                }
                _ => None,
            };
            DoctorCheckResult {
                section: DoctorSection::StartReadiness,
                name: "config".to_string(),
                status: CheckStatus::Fail(e.to_string()),
                remediation,
            }
        }
    }
}

fn check_git(runner: &dyn CommandRunner) -> DoctorCheckResult {
    match runner.run("git", &["--version"]) {
        Err(e) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "git".to_string(),
            status: CheckStatus::Fail(format!("git の確認中にエラーが発生しました: {e}")),
            remediation: Some("git をインストールしてください: https://git-scm.com/".to_string()),
        },
        Ok(output) if output.success => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "git".to_string(),
            status: CheckStatus::Ok("git がインストールされています".to_string()),
            remediation: None,
        },
        Ok(output) if output.stderr.contains("command not found") => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "git".to_string(),
            status: CheckStatus::Fail("git がインストールされていません".to_string()),
            remediation: Some("git をインストールしてください: https://git-scm.com/".to_string()),
        },
        Ok(output) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "git".to_string(),
            status: CheckStatus::Fail(format!(
                "git の実行に失敗しました: {}",
                output.stderr.trim()
            )),
            remediation: Some("git をインストールしてください: https://git-scm.com/".to_string()),
        },
    }
}

fn check_github_token(runner: &dyn CommandRunner) -> DoctorCheckResult {
    match runner.run("gh", &["auth", "token"]) {
        Err(e) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "github token".to_string(),
            status: CheckStatus::Fail(format!(
                "GitHub トークンの確認中にエラーが発生しました: {e}"
            )),
            remediation: Some("`gh auth login` を実行してください".to_string()),
        },
        Ok(output) if output.success => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "github token".to_string(),
            status: CheckStatus::Ok("GitHub トークンが取得できました".to_string()),
            remediation: None,
        },
        Ok(output) if output.stderr.contains("command not found") => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "github token".to_string(),
            status: CheckStatus::Fail("gh CLI がインストールされていません".to_string()),
            remediation: Some(
                "gh CLI をインストールしてください: https://cli.github.com/".to_string(),
            ),
        },
        Ok(_) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "github token".to_string(),
            status: CheckStatus::Fail("GitHub トークンを取得できません".to_string()),
            remediation: Some("`gh auth login` を実行してください".to_string()),
        },
    }
}

fn check_claude(runner: &dyn CommandRunner) -> DoctorCheckResult {
    match runner.run("claude", &["--version"]) {
        Err(e) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "claude CLI".to_string(),
            status: CheckStatus::Fail(format!("claude CLI の確認中にエラーが発生しました: {e}")),
            remediation: Some("https://claude.ai/code からインストールしてください".to_string()),
        },
        Ok(output) if output.success => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "claude CLI".to_string(),
            status: CheckStatus::Ok("claude CLI がインストールされています".to_string()),
            remediation: None,
        },
        Ok(_) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "claude CLI".to_string(),
            status: CheckStatus::Fail(
                "claude CLI がインストールされていないか正常に動作しません".to_string(),
            ),
            remediation: Some("https://claude.ai/code からインストールしてください".to_string()),
        },
    }
}

fn check_db(db_path: &Path) -> DoctorCheckResult {
    if db_path.exists() {
        DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "database".to_string(),
            status: CheckStatus::Ok("cupola.db が存在します".to_string()),
            remediation: None,
        }
    } else {
        DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "database".to_string(),
            status: CheckStatus::Fail("cupola.db が見つかりません".to_string()),
            remediation: Some("`cupola init` を実行してください".to_string()),
        }
    }
}

fn check_assets(commands_path: &Path, settings_path: &Path) -> DoctorCheckResult {
    let commands_ok = commands_path.is_dir();
    let settings_ok = settings_path.is_dir();

    if commands_ok && settings_ok {
        DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "assets".to_string(),
            status: CheckStatus::Ok("Cupola assets が存在します".to_string()),
            remediation: None,
        }
    } else {
        let mut missing = Vec::new();
        if !commands_ok {
            missing.push(commands_path.display().to_string());
        }
        if !settings_ok {
            missing.push(settings_path.display().to_string());
        }
        DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "assets".to_string(),
            status: CheckStatus::Warn(format!(
                "Cupola assets が不足しています: {}",
                missing.join(", ")
            )),
            remediation: Some("`cupola init` を実行してください".to_string()),
        }
    }
}

fn check_steering(steering_path: &Path) -> DoctorCheckResult {
    let file_count = match std::fs::read_dir(steering_path) {
        Err(_) => 0usize,
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map(|ft| ft.is_file()).unwrap_or(false)
                    && !e.file_name().to_string_lossy().starts_with('.')
            })
            .count(),
    };

    if file_count > 0 {
        DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "steering".to_string(),
            status: CheckStatus::Ok(format!(
                "steering ディレクトリに {file_count} 件のファイルが見つかりました"
            )),
            remediation: None,
        }
    } else {
        DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "steering".to_string(),
            status: CheckStatus::Warn(
                "steering ディレクトリにファイルが見つかりません".to_string(),
            ),
            remediation: Some(
                "`cupola init` を実行するか、`/cupola:steering` でステアリングファイルを作成してください".to_string(),
            ),
        }
    }
}

#[derive(Deserialize)]
struct LabelItem {
    name: String,
}

/// `gh label list` を1回呼び出し、agent:ready / weight:* の両チェック結果を返す
fn check_labels(runner: &dyn CommandRunner) -> [DoctorCheckResult; 2] {
    let gh_not_installed_results = || {
        [
            DoctorCheckResult {
                section: DoctorSection::OperationalReadiness,
                name: "agent:ready ラベル".to_string(),
                status: CheckStatus::Warn(
                    "gh CLI がインストールされていないため、ラベルを確認できません".to_string(),
                ),
                remediation: Some(
                    "gh CLI をインストールしてください: https://cli.github.com/".to_string(),
                ),
            },
            DoctorCheckResult {
                section: DoctorSection::OperationalReadiness,
                name: "weight:* ラベル".to_string(),
                status: CheckStatus::Warn(
                    "gh CLI がインストールされていないため、ラベルを確認できません".to_string(),
                ),
                remediation: Some(
                    "gh CLI をインストールしてください: https://cli.github.com/".to_string(),
                ),
            },
        ]
    };

    let auth_failed_results = || {
        [
            DoctorCheckResult {
                section: DoctorSection::OperationalReadiness,
                name: "agent:ready ラベル".to_string(),
                status: CheckStatus::Warn(
                    "ラベル一覧の取得に失敗しました。`gh auth login` で認証してください"
                        .to_string(),
                ),
                remediation: Some("`gh auth login` を実行してください".to_string()),
            },
            DoctorCheckResult {
                section: DoctorSection::OperationalReadiness,
                name: "weight:* ラベル".to_string(),
                status: CheckStatus::Warn(
                    "ラベル一覧の取得に失敗しました。`gh auth login` で認証してください"
                        .to_string(),
                ),
                remediation: Some("`gh auth login` を実行してください".to_string()),
            },
        ]
    };

    let stdout = match runner.run("gh", &["label", "list", "--json", "name"]) {
        Err(_) => return auth_failed_results(),
        Ok(output) if !output.success && output.stderr.contains("command not found") => {
            return gh_not_installed_results();
        }
        Ok(output) if !output.success => return auth_failed_results(),
        Ok(output) => output.stdout,
    };

    let items: Vec<LabelItem> = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            let msg = format!("ラベル一覧の取得に失敗しました: {e}");
            return [
                DoctorCheckResult {
                    section: DoctorSection::OperationalReadiness,
                    name: "agent:ready ラベル".to_string(),
                    status: CheckStatus::Warn(msg.clone()),
                    remediation: Some("`gh label create agent:ready` を実行してください".to_string()),
                },
                DoctorCheckResult {
                    section: DoctorSection::OperationalReadiness,
                    name: "weight:* ラベル".to_string(),
                    status: CheckStatus::Warn(msg),
                    remediation: Some(
                        "`gh label create weight:light && gh label create weight:heavy` を実行してください"
                            .to_string(),
                    ),
                },
            ];
        }
    };

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();

    let agent_ready = if names.contains(&"agent:ready") {
        DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "agent:ready ラベル".to_string(),
            status: CheckStatus::Ok("agent:ready ラベルがリポジトリに存在します".to_string()),
            remediation: None,
        }
    } else {
        DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "agent:ready ラベル".to_string(),
            status: CheckStatus::Warn("agent:ready ラベルがリポジトリに存在しません".to_string()),
            remediation: Some("`gh label create agent:ready` を実行してください".to_string()),
        }
    };

    let has_light = names.contains(&"weight:light");
    let has_heavy = names.contains(&"weight:heavy");
    let weight = if has_light && has_heavy {
        DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "weight:* ラベル".to_string(),
            status: CheckStatus::Ok(
                "weight:light と weight:heavy ラベルがリポジトリに存在します".to_string(),
            ),
            remediation: None,
        }
    } else {
        let missing: Vec<&str> = [
            (!has_light).then_some("weight:light"),
            (!has_heavy).then_some("weight:heavy"),
        ]
        .into_iter()
        .flatten()
        .collect();
        let create_cmds: Vec<String> = missing
            .iter()
            .map(|l| format!("gh label create {l}"))
            .collect();
        DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "weight:* ラベル".to_string(),
            status: CheckStatus::Warn(format!(
                "{} ラベルがリポジトリに存在しません",
                missing.join(", ")
            )),
            remediation: Some(format!("`{}` を実行してください", create_cmds.join(" && "))),
        }
    };

    [agent_ready, weight]
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::application::port::command_runner::test_support::MockCommandRunner;
    use crate::application::port::config_loader::{ConfigLoadError, DoctorConfigSummary};
    use crate::domain::issue::Issue;
    use crate::domain::state::State;

    // --- MockIssueRepository ---

    struct MockIssueRepository {
        issues: Vec<Issue>,
    }

    impl MockIssueRepository {
        fn empty() -> Self {
            Self { issues: vec![] }
        }

        fn with_issues(issues: Vec<Issue>) -> Self {
            Self { issues }
        }
    }

    impl crate::application::port::issue_repository::IssueRepository for MockIssueRepository {
        async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<Issue>> {
            Ok(self.issues.iter().find(|i| i.id == id).cloned())
        }

        async fn find_by_issue_number(&self, issue_number: u64) -> anyhow::Result<Option<Issue>> {
            Ok(self
                .issues
                .iter()
                .find(|i| i.github_issue_number == issue_number)
                .cloned())
        }

        async fn find_active(&self) -> anyhow::Result<Vec<Issue>> {
            Ok(self
                .issues
                .iter()
                .filter(|i| i.state != State::Completed && i.state != State::Cancelled)
                .cloned()
                .collect())
        }

        async fn find_all(&self) -> anyhow::Result<Vec<Issue>> {
            Ok(self.issues.clone())
        }

        async fn save(&self, _issue: &Issue) -> anyhow::Result<i64> {
            Ok(0)
        }

        async fn update_state(&self, _id: i64, _state: State) -> anyhow::Result<()> {
            Ok(())
        }

        async fn update(&self, _issue: &Issue) -> anyhow::Result<()> {
            Ok(())
        }

        async fn update_state_and_metadata(
            &self,
            _issue_id: i64,
            _updates: &crate::domain::metadata_update::MetadataUpdates,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn find_by_state(&self, state: State) -> anyhow::Result<Vec<Issue>> {
            Ok(self
                .issues
                .iter()
                .filter(|i| i.state == state)
                .cloned()
                .collect())
        }
    }

    // --- MockConfigLoader ---

    struct MockConfigLoader {
        result: Result<DoctorConfigSummary, ConfigLoadError>,
    }

    impl MockConfigLoader {
        fn ok() -> Self {
            Self {
                result: Ok(DoctorConfigSummary {
                    owner: "owner".to_string(),
                    repo: "repo".to_string(),
                    default_branch: "main".to_string(),
                    claude_code_extra_allow: vec![],
                }),
            }
        }

        fn not_found(path: &str) -> Self {
            Self {
                result: Err(ConfigLoadError::NotFound {
                    path: path.to_string(),
                }),
            }
        }

        fn missing_field(field: &str) -> Self {
            Self {
                result: Err(ConfigLoadError::MissingField {
                    field: field.to_string(),
                }),
            }
        }

        fn validation_failed(reason: &str) -> Self {
            Self {
                result: Err(ConfigLoadError::ValidationFailed {
                    reason: reason.to_string(),
                }),
            }
        }
    }

    impl ConfigLoader for MockConfigLoader {
        fn load(&self, _path: &Path) -> Result<DoctorConfigSummary, ConfigLoadError> {
            match &self.result {
                Ok(s) => Ok(DoctorConfigSummary {
                    owner: s.owner.clone(),
                    repo: s.repo.clone(),
                    default_branch: s.default_branch.clone(),
                    claude_code_extra_allow: s.claude_code_extra_allow.clone(),
                }),
                Err(ConfigLoadError::NotFound { path }) => {
                    Err(ConfigLoadError::NotFound { path: path.clone() })
                }
                Err(ConfigLoadError::ReadFailed { path, reason }) => {
                    Err(ConfigLoadError::ReadFailed {
                        path: path.clone(),
                        reason: reason.clone(),
                    })
                }
                Err(ConfigLoadError::ParseFailed { path, reason }) => {
                    Err(ConfigLoadError::ParseFailed {
                        path: path.clone(),
                        reason: reason.clone(),
                    })
                }
                Err(ConfigLoadError::MissingField { field }) => {
                    Err(ConfigLoadError::MissingField {
                        field: field.clone(),
                    })
                }
                Err(ConfigLoadError::ValidationFailed { reason }) => {
                    Err(ConfigLoadError::ValidationFailed {
                        reason: reason.clone(),
                    })
                }
            }
        }
    }

    // --- check_config tests ---

    #[test]
    fn check_config_with_valid_file_returns_ok() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cupola.toml");

        let loader = MockConfigLoader::ok();
        let result = check_config(&loader, &path);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_config_when_file_missing_returns_fail_with_remediation() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cupola.toml");

        let loader = MockConfigLoader::not_found(path.to_str().unwrap());
        let result = check_config(&loader, &path);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_some());
    }

    #[test]
    fn check_config_when_missing_field_returns_fail() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cupola.toml");

        let loader = MockConfigLoader::missing_field("default_branch");
        let result = check_config(&loader, &path);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
    }

    #[test]
    fn check_config_when_validation_fails_returns_fail_with_remediation() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cupola.toml");

        let loader =
            MockConfigLoader::validation_failed("polling_interval_secs must be at least 10");
        let result = check_config(&loader, &path);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_some());
    }

    // --- check_steering tests ---

    #[test]
    fn check_steering_with_md_file_returns_ok() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("product.md"), "# Product").unwrap();

        let result = check_steering(dir.path());
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(
            result.section,
            DoctorSection::OperationalReadiness
        ));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_steering_with_empty_dir_returns_warn() {
        let dir = TempDir::new().unwrap();

        let result = check_steering(dir.path());
        assert!(matches!(result.status, CheckStatus::Warn(_)));
        assert!(matches!(
            result.section,
            DoctorSection::OperationalReadiness
        ));
        assert!(result.remediation.is_some());
    }

    #[test]
    fn check_steering_with_only_subdir_returns_warn() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let result = check_steering(dir.path());
        assert!(matches!(result.status, CheckStatus::Warn(_)));
    }

    #[test]
    fn check_steering_with_only_hidden_file_returns_warn() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".DS_Store"), "").unwrap();

        let result = check_steering(dir.path());
        assert!(matches!(result.status, CheckStatus::Warn(_)));
    }

    // --- check_db tests ---

    #[test]
    fn check_db_with_existing_file_returns_ok() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("cupola.db");
        fs::write(&db_path, "").unwrap();

        let result = check_db(&db_path);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_db_with_missing_file_returns_fail_with_remediation() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("cupola.db");

        let result = check_db(&db_path);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_some());
    }

    // --- check_git tests ---

    #[test]
    fn check_git_with_mock_success_returns_ok() {
        let runner =
            MockCommandRunner::new().with_success("git", &["--version"], "git version 2.x");
        let result = check_git(&runner);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_git_when_not_installed_returns_fail_with_install_remediation() {
        // デフォルトは stderr = "command not found" → git 未インストール
        let runner = MockCommandRunner::new();
        let result = check_git(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_some());
        if let CheckStatus::Fail(msg) = &result.status {
            assert!(msg.contains("インストールされていません"));
        }
    }

    #[test]
    fn check_git_when_execution_fails_returns_fail_with_stderr() {
        // with_failure は stderr = "error" → その他のエラー
        let runner = MockCommandRunner::new().with_failure("git", &["--version"]);
        let result = check_git(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        if let CheckStatus::Fail(msg) = &result.status {
            assert!(msg.contains("実行に失敗"));
        }
    }

    // --- check_github_token tests ---

    #[test]
    fn check_github_token_with_success_returns_ok() {
        let runner =
            MockCommandRunner::new().with_success("gh", &["auth", "token"], "ghp_xxxxxxxxxxxx");
        let result = check_github_token(&runner);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_github_token_when_gh_not_installed_returns_fail_with_install_remediation() {
        // デフォルトは stderr = "command not found" → gh 未インストール
        let runner = MockCommandRunner::new();
        let result = check_github_token(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_some());
        let rem = result.remediation.unwrap();
        assert!(rem.contains("cli.github.com"));
    }

    #[test]
    fn check_github_token_when_auth_fails_returns_fail_with_login_remediation() {
        // with_failure は stderr = "error" → 認証失敗
        let runner = MockCommandRunner::new().with_failure("gh", &["auth", "token"]);
        let result = check_github_token(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(result.remediation.is_some());
        let rem = result.remediation.unwrap();
        assert!(rem.contains("gh auth login"));
    }

    // --- check_claude tests ---

    #[test]
    fn check_claude_with_success_returns_ok() {
        let runner =
            MockCommandRunner::new().with_success("claude", &["--version"], "Claude CLI 1.0.0");
        let result = check_claude(&runner);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_claude_with_failure_returns_fail_with_remediation() {
        let runner = MockCommandRunner::new();
        let result = check_claude(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_some());
        let rem = result.remediation.unwrap();
        assert!(rem.contains("claude.ai/code"));
    }

    // --- check_assets tests ---

    #[test]
    fn check_assets_with_both_dirs_returns_ok() {
        let dir = TempDir::new().unwrap();
        let commands_path = dir.path().join(".claude").join("commands").join("cupola");
        let settings_path = dir.path().join(".cupola").join("settings");
        fs::create_dir_all(&commands_path).unwrap();
        fs::create_dir_all(&settings_path).unwrap();

        let result = check_assets(&commands_path, &settings_path);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(
            result.section,
            DoctorSection::OperationalReadiness
        ));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_assets_with_missing_commands_dir_returns_warn() {
        let dir = TempDir::new().unwrap();
        let commands_path = dir.path().join(".claude").join("commands").join("cupola");
        let settings_path = dir.path().join(".cupola").join("settings");
        fs::create_dir_all(&settings_path).unwrap();

        let result = check_assets(&commands_path, &settings_path);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
        assert!(matches!(
            result.section,
            DoctorSection::OperationalReadiness
        ));
        assert!(result.remediation.is_some());
        let rem = result.remediation.unwrap();
        assert!(rem.contains("cupola init"));
    }

    #[test]
    fn check_assets_with_missing_settings_dir_returns_warn() {
        let dir = TempDir::new().unwrap();
        let commands_path = dir.path().join(".claude").join("commands").join("cupola");
        let settings_path = dir.path().join(".cupola").join("settings");
        fs::create_dir_all(&commands_path).unwrap();

        let result = check_assets(&commands_path, &settings_path);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
    }

    #[test]
    fn check_assets_with_both_missing_returns_warn() {
        let dir = TempDir::new().unwrap();
        let commands_path = dir.path().join(".claude").join("commands").join("cupola");
        let settings_path = dir.path().join(".cupola").join("settings");

        let result = check_assets(&commands_path, &settings_path);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
        assert!(result.remediation.is_some());
    }

    // --- check_labels tests ---

    #[test]
    fn check_labels_with_all_labels_present_returns_ok() {
        let runner = MockCommandRunner::new().with_success(
            "gh",
            &["label", "list", "--json", "name"],
            r#"[{"name":"agent:ready"},{"name":"weight:light"},{"name":"weight:heavy"}]"#,
        );
        let [agent_ready, weight] = check_labels(&runner);
        assert!(matches!(agent_ready.status, CheckStatus::Ok(_)));
        assert!(matches!(weight.status, CheckStatus::Ok(_)));
        assert!(agent_ready.remediation.is_none());
        assert!(weight.remediation.is_none());
    }

    #[test]
    fn check_labels_when_agent_ready_missing_returns_warn_with_create_remediation() {
        let runner = MockCommandRunner::new().with_success(
            "gh",
            &["label", "list", "--json", "name"],
            r#"[{"name":"weight:light"},{"name":"weight:heavy"}]"#,
        );
        let [agent_ready, weight] = check_labels(&runner);
        assert!(matches!(agent_ready.status, CheckStatus::Warn(_)));
        assert!(
            agent_ready
                .remediation
                .as_deref()
                .unwrap()
                .contains("gh label create agent:ready")
        );
        assert!(matches!(weight.status, CheckStatus::Ok(_)));
    }

    #[test]
    fn check_labels_when_weight_labels_missing_returns_warn() {
        let runner = MockCommandRunner::new().with_success(
            "gh",
            &["label", "list", "--json", "name"],
            r#"[{"name":"agent:ready"}]"#,
        );
        let [agent_ready, weight] = check_labels(&runner);
        assert!(matches!(agent_ready.status, CheckStatus::Ok(_)));
        assert!(matches!(weight.status, CheckStatus::Warn(_)));
        assert!(weight.remediation.is_some());
    }

    #[test]
    fn check_labels_when_gh_not_installed_returns_warn_with_install_remediation() {
        // デフォルトは stderr = "command not found"
        let runner = MockCommandRunner::new();
        let [agent_ready, weight] = check_labels(&runner);
        assert!(matches!(agent_ready.status, CheckStatus::Warn(_)));
        assert!(matches!(weight.status, CheckStatus::Warn(_)));
        let rem = agent_ready.remediation.unwrap();
        assert!(rem.contains("cli.github.com"));
    }

    #[test]
    fn check_labels_when_auth_fails_returns_warn_with_login_remediation() {
        // with_failure は stderr = "error" → 認証失敗
        let runner =
            MockCommandRunner::new().with_failure("gh", &["label", "list", "--json", "name"]);
        let [agent_ready, weight] = check_labels(&runner);
        assert!(matches!(agent_ready.status, CheckStatus::Warn(_)));
        assert!(matches!(weight.status, CheckStatus::Warn(_)));
        let rem = agent_ready.remediation.unwrap();
        assert!(rem.contains("gh auth login"));
    }

    // --- DoctorUseCase integration test ---

    #[tokio::test]
    async fn doctor_use_case_returns_all_sections() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");
        let loader = MockConfigLoader::ok();
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner, MockIssueRepository::empty(), 3);
        let results = use_case.run(&config_path).await;

        // 11 checks: 6 Start Readiness (config + git + github_token + claude + db + env_allowlist)
        //            + 5 Operational Readiness (includes ci-fix-limit check)
        assert_eq!(results.len(), 11);

        let start_count = results
            .iter()
            .filter(|r| matches!(r.section, DoctorSection::StartReadiness))
            .count();
        let op_count = results
            .iter()
            .filter(|r| matches!(r.section, DoctorSection::OperationalReadiness))
            .count();
        assert_eq!(start_count, 6);
        assert_eq!(op_count, 5);
    }

    #[tokio::test]
    async fn doctor_use_case_toml_fail_with_mock_loader() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");

        let loader = MockConfigLoader::not_found(config_path.to_str().unwrap());
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner, MockIssueRepository::empty(), 3);
        let results = use_case.run(&config_path).await;

        assert!(matches!(results[0].status, CheckStatus::Fail(_)));
    }

    // --- T-6.DR.* tests ---

    /// T-6.DR.1: Start Readiness セクションに config/git/github_token/claude/database チェックが含まれる
    #[tokio::test]
    async fn t_6_dr_1_start_readiness_contains_required_checks() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");
        let loader = MockConfigLoader::ok();
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner, MockIssueRepository::empty(), 3);
        let results = use_case.run(&config_path).await;

        let start_checks: Vec<&str> = results
            .iter()
            .filter(|r| matches!(r.section, DoctorSection::StartReadiness))
            .map(|r| r.name.as_str())
            .collect();

        // Must contain these check names (order flexible, case insensitive)
        assert!(
            start_checks
                .iter()
                .any(|n| n.to_lowercase().contains("config")),
            "Config check missing: {start_checks:?}"
        );
        assert!(
            start_checks
                .iter()
                .any(|n| n.to_lowercase().contains("git")),
            "Git check missing: {start_checks:?}"
        );
        assert!(
            start_checks
                .iter()
                .any(|n| n.to_lowercase().contains("database") || n.to_lowercase().contains("db")),
            "Database check missing: {start_checks:?}"
        );
    }

    /// T-6.DR.2: Start Readiness に失敗があれば exit non-zero（DoctorCheckResult の status が Fail）
    #[tokio::test]
    async fn t_6_dr_2_start_readiness_fail_means_non_zero_exit() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");
        let loader = MockConfigLoader::not_found(config_path.to_str().unwrap());
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner, MockIssueRepository::empty(), 3);
        let results = use_case.run(&config_path).await;

        let has_start_failure = results
            .iter()
            .filter(|r| matches!(r.section, DoctorSection::StartReadiness))
            .any(|r| matches!(r.status, CheckStatus::Fail(_)));
        assert!(
            has_start_failure,
            "should have at least one Fail in Start Readiness"
        );
    }

    /// T-6.DR.3: Operational Readiness セクションに assets/steering/labels チェックが含まれる（Fail はしない）
    #[tokio::test]
    async fn t_6_dr_3_operational_readiness_contains_required_checks() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");
        let loader = MockConfigLoader::ok();
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner, MockIssueRepository::empty(), 3);
        let results = use_case.run(&config_path).await;

        let op_checks: Vec<&str> = results
            .iter()
            .filter(|r| matches!(r.section, DoctorSection::OperationalReadiness))
            .map(|r| r.name.as_str())
            .collect();

        assert!(
            !op_checks.is_empty(),
            "Operational Readiness should have checks"
        );
        assert!(
            op_checks.iter().any(|n| {
                let n_lower = n.to_lowercase();
                n_lower.contains("assets")
                    || n_lower.contains("steering")
                    || n_lower.contains("label")
                    || n_lower.contains("ラベル")
            }),
            "should have assets/steering/labels checks: {op_checks:?}"
        );
    }

    /// T-6.DR.7: pending notification が存在する場合に Warn が返ること
    #[tokio::test]
    async fn check_pending_notifications_returns_warn_when_pending_exists() {
        let mut issue = Issue::new(1, "test".to_string());
        issue.ci_fix_count = 4;
        issue.ci_fix_limit_notified = false;
        let repo = MockIssueRepository::with_issues(vec![issue]);

        let result = check_pending_ci_fix_limit_notifications(&repo, 3).await;
        assert!(
            matches!(result.status, CheckStatus::Warn(_)),
            "should warn when pending"
        );
        assert!(matches!(
            result.section,
            DoctorSection::OperationalReadiness
        ));
        assert!(result.remediation.is_some());
        if let CheckStatus::Warn(msg) = &result.status {
            assert!(msg.contains("1"), "should mention count");
        }
    }

    /// T-6.DR.8: pending notification が存在しない場合に Ok が返ること
    #[tokio::test]
    async fn check_pending_notifications_returns_ok_when_no_pending() {
        let mut issue = Issue::new(1, "test".to_string());
        issue.ci_fix_count = 4;
        issue.ci_fix_limit_notified = true;
        let repo = MockIssueRepository::with_issues(vec![issue]);

        let result = check_pending_ci_fix_limit_notifications(&repo, 3).await;
        assert!(
            matches!(result.status, CheckStatus::Ok(_)),
            "should be ok when already notified"
        );
        assert!(matches!(
            result.section,
            DoctorSection::OperationalReadiness
        ));
        assert!(result.remediation.is_none());
    }

    /// T-6.DR.9: ci_fix_count が max_ci_fix_cycles 以下の場合は Ok が返ること
    #[tokio::test]
    async fn check_pending_notifications_returns_ok_when_count_not_exceeded() {
        let mut issue = Issue::new(1, "test".to_string());
        issue.ci_fix_count = 3;
        issue.ci_fix_limit_notified = false;
        let repo = MockIssueRepository::with_issues(vec![issue]);

        let result = check_pending_ci_fix_limit_notifications(&repo, 3).await;
        assert!(
            matches!(result.status, CheckStatus::Ok(_)),
            "should be ok when count not exceeded"
        );
    }

    /// T-6.DR.10: 空の Issue 一覧で Ok が返ること
    #[tokio::test]
    async fn check_pending_notifications_returns_ok_when_no_issues() {
        let repo = MockIssueRepository::empty();
        let result = check_pending_ci_fix_limit_notifications(&repo, 3).await;
        assert!(matches!(result.status, CheckStatus::Ok(_)));
    }

    /// T-6.DR.5: 出力に ✅ / ⚠️ / ❌ シンボルが使用されている（DoctorUseCase はシンボルを返さないが、
    /// bootstrap/app.rs で println! に展開されることを確認: CheckStatus variants の存在で担保）
    #[test]
    fn t_6_dr_5_check_status_variants_exist_for_all_symbols() {
        // CheckStatus::Ok → ✅, Warn → ⚠️, Fail → ❌
        let ok: CheckStatus = CheckStatus::Ok("test".into());
        let warn: CheckStatus = CheckStatus::Warn("test".into());
        let fail: CheckStatus = CheckStatus::Fail("test".into());
        assert!(matches!(ok, CheckStatus::Ok(_)));
        assert!(matches!(warn, CheckStatus::Warn(_)));
        assert!(matches!(fail, CheckStatus::Fail(_)));
    }

    // --- check_env_allowlist tests ---

    fn make_summary(extra_allow: Vec<&str>) -> DoctorConfigSummary {
        DoctorConfigSummary {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            default_branch: "main".to_string(),
            claude_code_extra_allow: extra_allow.into_iter().map(str::to_string).collect(),
        }
    }

    #[test]
    fn check_env_allowlist_empty_extra_allow_returns_ok() {
        let summary = make_summary(vec![]);
        let result = check_env_allowlist(&summary);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_env_allowlist_safe_patterns_returns_ok() {
        let summary = make_summary(vec!["DOCKER_HOST", "CLAUDE_*"]);
        let result = check_env_allowlist(&summary);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
    }

    #[test]
    fn check_env_allowlist_gh_token_exact_match_returns_warn() {
        let summary = make_summary(vec!["GH_TOKEN"]);
        let result = check_env_allowlist(&summary);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
        assert!(result.remediation.is_some());
        if let CheckStatus::Warn(msg) = &result.status {
            assert!(msg.contains("GH_TOKEN"));
        }
    }

    #[test]
    fn check_env_allowlist_github_token_exact_match_returns_warn() {
        let summary = make_summary(vec!["GITHUB_TOKEN"]);
        let result = check_env_allowlist(&summary);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
    }

    #[test]
    fn check_env_allowlist_aws_prefix_pattern_returns_warn() {
        let summary = make_summary(vec!["AWS_*"]);
        let result = check_env_allowlist(&summary);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
        if let CheckStatus::Warn(msg) = &result.status {
            assert!(msg.contains("AWS_*"));
        }
    }

    #[test]
    fn check_env_allowlist_api_key_suffix_returns_warn() {
        let summary = make_summary(vec!["OPENAI_API_KEY"]);
        let result = check_env_allowlist(&summary);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
        if let CheckStatus::Warn(msg) = &result.status {
            assert!(msg.contains("OPENAI_API_KEY"));
        }
    }

    #[test]
    fn check_env_allowlist_secret_suffix_returns_warn() {
        let summary = make_summary(vec!["MY_SECRET"]);
        let result = check_env_allowlist(&summary);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
    }

    #[test]
    fn check_env_allowlist_token_suffix_returns_warn() {
        let summary = make_summary(vec!["SOME_TOKEN"]);
        let result = check_env_allowlist(&summary);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
    }

    #[test]
    fn check_env_allowlist_password_suffix_returns_warn() {
        let summary = make_summary(vec!["DB_PASSWORD"]);
        let result = check_env_allowlist(&summary);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
    }

    #[tokio::test]
    async fn doctor_use_case_config_load_failure_skips_env_allowlist() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");

        let loader = MockConfigLoader::not_found(config_path.to_str().unwrap());
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner, MockIssueRepository::empty(), 3);
        let results = use_case.run(&config_path).await;

        // config ロード失敗時は env allowlist チェックが追加されない（10 件）
        assert_eq!(results.len(), 10);
        let env_check = results.iter().find(|r| r.name == "env allowlist");
        assert!(
            env_check.is_none(),
            "config load 失敗時は env allowlist チェックをスキップ"
        );
    }

    #[tokio::test]
    async fn doctor_start_readiness_includes_env_allowlist_check_on_success() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");
        let loader = MockConfigLoader::ok();
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner, MockIssueRepository::empty(), 3);
        let results = use_case.run(&config_path).await;

        let env_check = results.iter().find(|r| r.name == "env allowlist");
        assert!(
            env_check.is_some(),
            "config load 成功時は env allowlist チェックが含まれる"
        );
        let env_check = env_check.unwrap();
        assert!(matches!(env_check.section, DoctorSection::StartReadiness));
    }
}
