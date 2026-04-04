use std::path::Path;

use serde::Deserialize;

use crate::application::port::command_runner::CommandRunner;
use crate::application::port::config_loader::ConfigLoader;

/// 個別チェックのステータス
pub enum CheckStatus {
    Ok(String),
    Warn(String),
    Fail(String),
}

/// 1 件のチェック結果
pub struct DoctorCheckResult {
    pub name: String,
    pub status: CheckStatus,
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
        // 他コマンドと同様に、DB/steering は固定パス .cupola 配下を参照する
        let steering_path = Path::new(".cupola").join("steering");
        let db_path = Path::new(".cupola").join("cupola.db");

        vec![
            check_toml(&self.config_loader, config_path),
            check_git(&self.command_runner),
            check_gh(&self.command_runner),
            check_gh_label(&self.command_runner),
            check_weight_labels(&self.command_runner),
            check_steering(&steering_path),
            check_db(&db_path),
        ]
    }
}

fn check_toml(config_loader: &dyn ConfigLoader, path: &Path) -> DoctorCheckResult {
    match config_loader.load(path) {
        Ok(_) => DoctorCheckResult {
            name: "cupola.toml".to_string(),
            status: CheckStatus::Ok("cupola.toml が正常に読み込まれました".to_string()),
        },
        Err(e) => DoctorCheckResult {
            name: "cupola.toml".to_string(),
            status: CheckStatus::Fail(e.to_string()),
        },
    }
}

fn check_git(runner: &dyn CommandRunner) -> DoctorCheckResult {
    match runner.run("git", &["--version"]) {
        Err(e) => DoctorCheckResult {
            name: "git".to_string(),
            status: CheckStatus::Fail(format!("git の確認中にエラーが発生しました: {e}")),
        },
        Ok(output) if output.success => DoctorCheckResult {
            name: "git".to_string(),
            status: CheckStatus::Ok("git がインストールされています".to_string()),
        },
        Ok(output) if output.stderr.contains("command not found") => DoctorCheckResult {
            name: "git".to_string(),
            status: CheckStatus::Fail(
                "git がインストールされていません。git をインストールしてください: https://git-scm.com/"
                    .to_string(),
            ),
        },
        Ok(_) => DoctorCheckResult {
            name: "git".to_string(),
            status: CheckStatus::Fail(
                "git が正常に動作しません。git をインストールしてください: https://git-scm.com/"
                    .to_string(),
            ),
        },
    }
}

enum GhPresence {
    NotInstalled,
    InstalledButUnauthorized,
    Ready,
}

fn detect_gh_presence(runner: &dyn CommandRunner) -> GhPresence {
    match runner.run("gh", &["--version"]) {
        Err(_) => GhPresence::InstalledButUnauthorized,
        Ok(output) if !output.success => {
            if output.stderr.contains("command not found") {
                GhPresence::NotInstalled
            } else {
                GhPresence::InstalledButUnauthorized
            }
        }
        Ok(_) => match runner.run("gh", &["auth", "status"]) {
            Ok(output) if output.success => GhPresence::Ready,
            _ => GhPresence::InstalledButUnauthorized,
        },
    }
}

fn check_gh(runner: &dyn CommandRunner) -> DoctorCheckResult {
    match detect_gh_presence(runner) {
        GhPresence::NotInstalled => DoctorCheckResult {
            name: "gh".to_string(),
            status: CheckStatus::Fail(
                "gh CLI がインストールされていません。インストールしてください: brew install gh または https://cli.github.com/"
                    .to_string(),
            ),
        },
        GhPresence::InstalledButUnauthorized => DoctorCheckResult {
            name: "gh".to_string(),
            status: CheckStatus::Fail(
                "gh CLI の認証が完了していません。`gh auth login` を実行してください".to_string(),
            ),
        },
        GhPresence::Ready => DoctorCheckResult {
            name: "gh".to_string(),
            status: CheckStatus::Ok("gh CLI がインストールされ、認証済みです".to_string()),
        },
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
            name: "agent:ready ラベル".to_string(),
            status: CheckStatus::Fail(format!("ラベル一覧の取得に失敗しました: {e}")),
        },
        Ok(output) if !output.success => {
            if output.stderr.contains("command not found") {
                DoctorCheckResult {
                    name: "agent:ready ラベル".to_string(),
                    status: CheckStatus::Fail(
                        "gh CLI がインストールされていないため、ラベルを確認できません".to_string(),
                    ),
                }
            } else {
                DoctorCheckResult {
                    name: "agent:ready ラベル".to_string(),
                    status: CheckStatus::Fail(
                        "ラベル一覧の取得に失敗しました。gh の認証状態を確認してください"
                            .to_string(),
                    ),
                }
            }
        }
        Ok(output) => match parse_label_json(&output.stdout) {
            Ok(true) => DoctorCheckResult {
                name: "agent:ready ラベル".to_string(),
                status: CheckStatus::Ok(
                    "agent:ready ラベルがリポジトリに存在します".to_string(),
                ),
            },
            Ok(false) => DoctorCheckResult {
                name: "agent:ready ラベル".to_string(),
                status: CheckStatus::Fail(
                    "agent:ready ラベルがリポジトリに存在しません。`gh label create agent:ready` で作成してください".to_string(),
                ),
            },
            Err(e) => DoctorCheckResult {
                name: "agent:ready ラベル".to_string(),
                status: CheckStatus::Fail(format!("ラベル一覧の取得に失敗しました: {e}")),
            },
        },
    }
}

fn check_weight_labels(runner: &dyn CommandRunner) -> DoctorCheckResult {
    match runner.run("gh", &["label", "list", "--json", "name"]) {
        Err(e) => DoctorCheckResult {
            name: "weight:* ラベル".to_string(),
            status: CheckStatus::Fail(format!("ラベル一覧の取得に失敗しました: {e}")),
        },
        Ok(output) if !output.success => {
            if output.stderr.contains("command not found") {
                DoctorCheckResult {
                    name: "weight:* ラベル".to_string(),
                    status: CheckStatus::Fail(
                        "gh CLI がインストールされていないため、ラベルを確認できません".to_string(),
                    ),
                }
            } else {
                DoctorCheckResult {
                    name: "weight:* ラベル".to_string(),
                    status: CheckStatus::Fail(
                        "ラベル一覧の取得に失敗しました。gh の認証状態を確認してください"
                            .to_string(),
                    ),
                }
            }
        }
        Ok(output) => {
            let has_light = output.stdout.contains("\"weight:light\"");
            let has_heavy = output.stdout.contains("\"weight:heavy\"");
            if has_light && has_heavy {
                DoctorCheckResult {
                    name: "weight:* ラベル".to_string(),
                    status: CheckStatus::Ok(
                        "weight:light と weight:heavy ラベルがリポジトリに存在します".to_string(),
                    ),
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
                    name: "weight:* ラベル".to_string(),
                    status: CheckStatus::Warn(format!(
                        "{} ラベルがリポジトリに存在しません。`{}` を実行してください",
                        missing.join(", "),
                        create_cmds.join(" && ")
                    )),
                }
            }
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
            name: "steering".to_string(),
            status: CheckStatus::Ok(format!(
                "steering ディレクトリに {file_count} 件のファイルが見つかりました"
            )),
        }
    } else {
        DoctorCheckResult {
            name: "steering".to_string(),
            status: CheckStatus::Fail(
                "steering ディレクトリにファイルが見つかりません。`/kiro:steering` でステアリングファイルを作成してください".to_string(),
            ),
        }
    }
}

fn check_db(db_path: &Path) -> DoctorCheckResult {
    if db_path.exists() {
        DoctorCheckResult {
            name: "cupola.db".to_string(),
            status: CheckStatus::Ok("cupola.db が存在します".to_string()),
        }
    } else {
        DoctorCheckResult {
            name: "cupola.db".to_string(),
            status: CheckStatus::Fail(
                "cupola.db が見つかりません。`cupola init` を実行してください".to_string(),
            ),
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
            }
        }
    }

    // --- check_toml tests ---

    #[test]
    fn check_toml_with_valid_file_returns_ok() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cupola.toml");
        fs::write(
            &path,
            r#"owner = "o"\nrepo = "r"\ndefault_branch = "main"\n"#,
        )
        .unwrap();

        let loader = MockConfigLoader::ok();
        let result = check_toml(&loader, &path);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
    }

    #[test]
    fn check_toml_when_file_missing_returns_fail() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cupola.toml");

        let loader = MockConfigLoader::not_found(path.to_str().unwrap());
        let result = check_toml(&loader, &path);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn check_toml_when_missing_field_returns_fail() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cupola.toml");

        let loader = MockConfigLoader::missing_field("default_branch");
        let result = check_toml(&loader, &path);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    // --- check_steering tests ---

    #[test]
    fn check_steering_with_md_file_returns_ok() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("product.md"), "# Product").unwrap();

        let result = check_steering(dir.path());
        assert!(matches!(result.status, CheckStatus::Ok(_)));
    }

    #[test]
    fn check_steering_with_only_subdir_returns_fail() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let result = check_steering(dir.path());
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn check_steering_with_empty_dir_returns_fail() {
        let dir = TempDir::new().unwrap();

        let result = check_steering(dir.path());
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn check_steering_with_only_hidden_file_returns_fail() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".DS_Store"), "").unwrap();

        let result = check_steering(dir.path());
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    // --- check_db tests ---

    #[test]
    fn check_db_with_existing_file_returns_ok() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("cupola.db");
        fs::write(&db_path, "").unwrap();

        let result = check_db(&db_path);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
    }

    #[test]
    fn check_db_with_missing_file_returns_fail() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("cupola.db");

        let result = check_db(&db_path);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    // --- check_git tests with MockCommandRunner ---

    #[test]
    fn check_git_with_mock_success_returns_ok() {
        let runner =
            MockCommandRunner::new().with_success("git", &["--version"], "git version 2.x");
        let result = check_git(&runner);
        assert!(matches!(result.status, CheckStatus::Ok(_)));
    }

    #[test]
    fn check_git_with_mock_not_found_returns_fail() {
        // default response: success=false, stderr="command not found"
        let runner = MockCommandRunner::new();
        let result = check_git(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
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

    // --- DoctorUseCase integration test with mock ---

    #[test]
    fn doctor_use_case_all_ok_with_mock_loader() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");
        let loader = MockConfigLoader::ok();
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner);
        let results = use_case.run(&config_path);

        // toml check should be Ok (mock returns Ok)
        assert!(matches!(results[0].status, CheckStatus::Ok(_)));
        // 7 checks should be returned (toml, git, gh, agent:ready, weight:*, steering, db)
        assert_eq!(results.len(), 7);
    }

    #[test]
    fn doctor_use_case_toml_fail_with_mock_loader() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("cupola.toml");

        let loader = MockConfigLoader::not_found(config_path.to_str().unwrap());
        let runner = MockCommandRunner::new();
        let use_case = DoctorUseCase::new(loader, runner);
        let results = use_case.run(&config_path);

        // toml check should be Fail
        assert!(matches!(results[0].status, CheckStatus::Fail(_)));
    }

    #[test]
    fn check_git_with_mock_installed_but_failing_returns_fail() {
        let runner = MockCommandRunner::new().with_failure("git", &["--version"]);
        let result = check_git(&runner);
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }
}
