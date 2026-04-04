use std::path::Path;

use serde::Deserialize;

use crate::application::port::command_runner::CommandRunner;
use crate::application::port::config_loader::ConfigLoader;

/// 診断セクション: Start Readiness か Operational Readiness か
pub enum DoctorSection {
    StartReadiness,
    OperationalReadiness,
}

/// 個別チェックのステータス
pub enum CheckStatus {
    Ok(String),
    Warn(String),
    Fail(String),
}

/// 1 件のチェック結果
pub struct DoctorCheckResult {
    pub section: DoctorSection,
    pub name: String,
    pub status: CheckStatus,
    /// 修復方法（None の場合は修復不要）
    pub remediation: Option<String>,
}

pub struct DoctorUseCase<C: ConfigLoader, R: CommandRunner> {
    config_loader: C,
    command_runner: R,
}

impl<C: ConfigLoader, R: CommandRunner> DoctorUseCase<C, R> {
    pub fn new(config_loader: C, command_runner: R) -> Self {
        Self {
            config_loader,
            command_runner,
        }
    }

    pub fn run(&self, config_path: &Path) -> Vec<DoctorCheckResult> {
        let steering_path = Path::new(".cupola").join("steering");
        let db_path = Path::new(".cupola").join("cupola.db");
        let commands_path = Path::new(".claude").join("commands").join("cupola");
        let settings_path = Path::new(".cupola").join("settings");

        vec![
            // Start Readiness
            check_config(&self.config_loader, config_path),
            check_git(&self.command_runner),
            check_github_token(&self.command_runner),
            check_claude(&self.command_runner),
            check_db(&db_path),
            // Operational Readiness
            check_assets(&commands_path, &settings_path),
            check_steering(&steering_path),
            check_gh_label(&self.command_runner),
            check_weight_labels(&self.command_runner),
        ]
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
            remediation: Some(
                "git をインストールしてください: https://git-scm.com/".to_string(),
            ),
        },
        Ok(output) if output.success => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "git".to_string(),
            status: CheckStatus::Ok("git がインストールされています".to_string()),
            remediation: None,
        },
        Ok(_) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "git".to_string(),
            status: CheckStatus::Fail(
                "git がインストールされていません".to_string(),
            ),
            remediation: Some(
                "git をインストールしてください: https://git-scm.com/".to_string(),
            ),
        },
    }
}

fn check_github_token(runner: &dyn CommandRunner) -> DoctorCheckResult {
    match runner.run("gh", &["auth", "token"]) {
        Err(e) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "github token".to_string(),
            status: CheckStatus::Fail(format!("GitHub トークンの確認中にエラーが発生しました: {e}")),
            remediation: Some("`gh auth login` を実行してください".to_string()),
        },
        Ok(output) if output.success => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "github token".to_string(),
            status: CheckStatus::Ok("GitHub トークンが取得できました".to_string()),
            remediation: None,
        },
        Ok(_) => DoctorCheckResult {
            section: DoctorSection::StartReadiness,
            name: "github token".to_string(),
            status: CheckStatus::Fail(
                "GitHub トークンを取得できません".to_string(),
            ),
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
            remediation: Some(
                "https://claude.ai/code からインストールしてください".to_string(),
            ),
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
            remediation: Some(
                "https://claude.ai/code からインストールしてください".to_string(),
            ),
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
            status: CheckStatus::Fail(
                "cupola.db が見つかりません".to_string(),
            ),
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

fn parse_label_json(stdout: &str) -> Result<bool, String> {
    let items: Vec<LabelItem> =
        serde_json::from_str(stdout).map_err(|e| format!("JSON パースエラー: {e}"))?;
    Ok(items.iter().any(|item| item.name == "agent:ready"))
}

fn check_gh_label(runner: &dyn CommandRunner) -> DoctorCheckResult {
    match runner.run("gh", &["label", "list", "--json", "name"]) {
        Err(e) => DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "agent:ready ラベル".to_string(),
            status: CheckStatus::Warn(format!("ラベル一覧の取得に失敗しました: {e}")),
            remediation: Some(
                "`gh label create agent:ready` を実行してください".to_string(),
            ),
        },
        Ok(output) if !output.success => DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "agent:ready ラベル".to_string(),
            status: CheckStatus::Warn(
                "ラベル一覧の取得に失敗しました。gh の認証状態を確認してください".to_string(),
            ),
            remediation: Some(
                "`gh label create agent:ready` を実行してください".to_string(),
            ),
        },
        Ok(output) => match parse_label_json(&output.stdout) {
            Ok(true) => DoctorCheckResult {
                section: DoctorSection::OperationalReadiness,
                name: "agent:ready ラベル".to_string(),
                status: CheckStatus::Ok(
                    "agent:ready ラベルがリポジトリに存在します".to_string(),
                ),
                remediation: None,
            },
            Ok(false) => DoctorCheckResult {
                section: DoctorSection::OperationalReadiness,
                name: "agent:ready ラベル".to_string(),
                status: CheckStatus::Warn(
                    "agent:ready ラベルがリポジトリに存在しません".to_string(),
                ),
                remediation: Some(
                    "`gh label create agent:ready` を実行してください".to_string(),
                ),
            },
            Err(e) => DoctorCheckResult {
                section: DoctorSection::OperationalReadiness,
                name: "agent:ready ラベル".to_string(),
                status: CheckStatus::Warn(format!("ラベル一覧の取得に失敗しました: {e}")),
                remediation: Some(
                    "`gh label create agent:ready` を実行してください".to_string(),
                ),
            },
        },
    }
}

fn check_weight_labels(runner: &dyn CommandRunner) -> DoctorCheckResult {
    match runner.run("gh", &["label", "list", "--json", "name"]) {
        Err(e) => DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "weight:* ラベル".to_string(),
            status: CheckStatus::Warn(format!("ラベル一覧の取得に失敗しました: {e}")),
            remediation: Some(
                "`gh label create weight:light && gh label create weight:heavy` を実行してください"
                    .to_string(),
            ),
        },
        Ok(output) if !output.success => DoctorCheckResult {
            section: DoctorSection::OperationalReadiness,
            name: "weight:* ラベル".to_string(),
            status: CheckStatus::Warn(
                "ラベル一覧の取得に失敗しました。gh の認証状態を確認してください".to_string(),
            ),
            remediation: Some(
                "`gh label create weight:light && gh label create weight:heavy` を実行してください"
                    .to_string(),
            ),
        },
        Ok(output) => {
            let has_light = output.stdout.contains("\"weight:light\"");
            let has_heavy = output.stdout.contains("\"weight:heavy\"");
            if has_light && has_heavy {
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
                        missing.join(", "),
                    )),
                    remediation: Some(format!("`{}` を実行してください", create_cmds.join(" && "))),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::application::port::command_runner::test_support::MockCommandRunner;
    use crate::application::port::config_loader::{ConfigLoadError, DoctorConfigSummary};

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

        let loader = MockConfigLoader::validation_failed("polling_interval_secs must be at least 10");
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
        assert!(matches!(result.section, DoctorSection::OperationalReadiness));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_steering_with_empty_dir_returns_warn() {
        let dir = TempDir::new().unwrap();

        let result = check_steering(dir.path());
        assert!(matches!(result.status, CheckStatus::Warn(_)));
        assert!(matches!(result.section, DoctorSection::OperationalReadiness));
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
    fn check_git_with_mock_not_found_returns_fail_with_remediation() {
        let runner = MockCommandRunner::new();
        let result = check_git(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_some());
    }

    #[test]
    fn check_git_with_mock_installed_but_failing_returns_fail() {
        let runner = MockCommandRunner::new().with_failure("git", &["--version"]);
        let result = check_git(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    // --- check_github_token tests ---

    #[test]
    fn check_github_token_with_success_returns_ok() {
        let runner = MockCommandRunner::new()
            .with_success("gh", &["auth", "token"], "ghp_xxxxxxxxxxxx");
        let result = check_github_token(&runner);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_github_token_with_failure_returns_fail_with_remediation() {
        let runner = MockCommandRunner::new();
        let result = check_github_token(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(matches!(result.section, DoctorSection::StartReadiness));
        assert!(result.remediation.is_some());
        let rem = result.remediation.unwrap();
        assert!(rem.contains("gh auth login"));
    }

    #[test]
    fn check_github_token_with_explicit_failure_returns_fail() {
        let runner = MockCommandRunner::new().with_failure("gh", &["auth", "token"]);
        let result = check_github_token(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
        assert!(result.remediation.is_some());
    }

    // --- check_claude tests ---

    #[test]
    fn check_claude_with_success_returns_ok() {
        let runner = MockCommandRunner::new()
            .with_success("claude", &["--version"], "Claude CLI 1.0.0");
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
        assert!(matches!(result.section, DoctorSection::OperationalReadiness));
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
        assert!(matches!(result.section, DoctorSection::OperationalReadiness));
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

    // --- label JSON parse tests ---

    #[test]
    fn parse_label_json_with_agent_ready_returns_true() {
        let json = r#"[{"name":"agent:ready"}]"#;
        assert!(parse_label_json(json).unwrap());
    }

    #[test]
    fn parse_label_json_with_different_label_returns_false() {
        let json = r#"[{"name":"not-agent:ready"}]"#;
        assert!(!parse_label_json(json).unwrap());
    }

    #[test]
    fn parse_label_json_with_invalid_json_returns_err() {
        let json = "";
        assert!(parse_label_json(json).is_err());
    }

    #[test]
    fn parse_label_json_with_partial_match_returns_false() {
        let json = r#"[{"name":"agent:ready-extra"}]"#;
        assert!(!parse_label_json(json).unwrap());
    }

    // --- check_gh_label tests ---

    #[test]
    fn check_gh_label_with_agent_ready_present_returns_ok() {
        let runner = MockCommandRunner::new().with_success(
            "gh",
            &["label", "list", "--json", "name"],
            r#"[{"name":"agent:ready"}]"#,
        );
        let result = check_gh_label(&runner);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
        assert!(matches!(result.section, DoctorSection::OperationalReadiness));
        assert!(result.remediation.is_none());
    }

    #[test]
    fn check_gh_label_when_label_missing_returns_warn_not_fail() {
        let runner = MockCommandRunner::new().with_success(
            "gh",
            &["label", "list", "--json", "name"],
            r#"[{"name":"other-label"}]"#,
        );
        let result = check_gh_label(&runner);
        assert!(matches!(result.status, CheckStatus::Warn(_)));
        assert!(result.remediation.is_some());
    }

    // --- DoctorUseCase integration test ---

    #[test]
    fn doctor_use_case_returns_all_sections() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");
        let loader = MockConfigLoader::ok();
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner);
        let results = use_case.run(&config_path);

        // 9 checks: 5 Start Readiness + 4 Operational Readiness
        assert_eq!(results.len(), 9);

        let start_count = results
            .iter()
            .filter(|r| matches!(r.section, DoctorSection::StartReadiness))
            .count();
        let op_count = results
            .iter()
            .filter(|r| matches!(r.section, DoctorSection::OperationalReadiness))
            .count();
        assert_eq!(start_count, 5);
        assert_eq!(op_count, 4);
    }

    #[test]
    fn doctor_use_case_toml_fail_with_mock_loader() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");

        let loader = MockConfigLoader::not_found(config_path.to_str().unwrap());
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner);
        let results = use_case.run(&config_path);

        assert!(matches!(results[0].status, CheckStatus::Fail(_)));
    }
}
