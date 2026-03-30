use std::path::Path;

use anyhow::{Context, Result};

use crate::adapter::inbound::cli::{Cli, Command};
use crate::adapter::outbound::claude_code_process::ClaudeCodeProcess;
use crate::adapter::outbound::git_worktree_manager::GitWorktreeManager;
use crate::adapter::outbound::github_client_impl::GitHubClientImpl;
use crate::adapter::outbound::github_graphql_client::GraphQLClient;
use crate::adapter::outbound::github_rest_client::OctocrabRestClient;
use crate::adapter::outbound::sqlite_connection::SqliteConnection;
use crate::adapter::outbound::sqlite_execution_log_repository::SqliteExecutionLogRepository;
use crate::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
use crate::application::doctor_use_case::{CheckStatus, DoctorUseCase};
use crate::application::init_use_case::InitUseCase;
use crate::application::polling_use_case::PollingUseCase;
use crate::application::port::issue_repository::IssueRepository;
use crate::bootstrap::config_loader::{CliOverrides, load_toml};
use crate::bootstrap::logging::init_logging;
use crate::bootstrap::toml_config_loader::TomlConfigLoader;

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Run {
            polling_interval_secs,
            log_level,
            config,
        } => {
            let toml = load_toml(&config)
                .with_context(|| format!("failed to load config from {}", config.display()))?;

            let overrides = CliOverrides {
                polling_interval_secs,
                log_level,
            };
            let cfg = toml.into_config(&overrides);
            cfg.validate()
                .map_err(|e| anyhow::anyhow!("config validation failed: {e}"))?;

            // Initialize logging (hold guard for app lifetime)
            let _guard = init_logging(cfg.log_level, cfg.log_dir.as_deref());

            tracing::info!(
                owner = %cfg.owner,
                repo = %cfg.repo,
                polling_interval = cfg.polling_interval_secs,
                "starting cupola"
            );

            // Initialize SQLite
            let db_path = Path::new(".cupola/cupola.db");
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).context("failed to create .cupola directory")?;
            }
            let db = SqliteConnection::open(db_path)?;
            db.init_schema()?;

            // Resolve GitHub token
            let token = gh_token::get()?;

            // Build adapters
            let rest = OctocrabRestClient::new(token.clone(), cfg.owner.clone(), cfg.repo.clone())?;
            let graphql = GraphQLClient::new(token, cfg.owner.clone(), cfg.repo.clone());
            let github = GitHubClientImpl::new(rest, graphql);
            let issue_repo = SqliteIssueRepository::new(db.clone());
            let exec_log_repo = SqliteExecutionLogRepository::new(db);
            let claude_runner = ClaudeCodeProcess::new("claude");
            let worktree = GitWorktreeManager::new(".");

            // Build and run polling use case
            let mut polling = PollingUseCase::new(
                github,
                issue_repo,
                exec_log_repo,
                claude_runner,
                worktree,
                cfg,
            );

            polling.run().await
        }

        Command::Init => {
            let base_dir = std::env::current_dir().context("failed to get current directory")?;
            let uc = InitUseCase::new(base_dir);
            let report = uc.run()?;

            println!("cupola init completed:");
            println!(
                "  database: {}",
                if report.db_initialized {
                    "initialized"
                } else {
                    "skipped"
                }
            );
            println!(
                "  cupola.toml: {}",
                if report.toml_created {
                    "created"
                } else {
                    "skipped (already exists)"
                }
            );
            println!(
                "  steering templates: {}",
                if report.steering_copied {
                    "copied"
                } else {
                    "skipped"
                }
            );
            println!(
                "  .gitignore: {}",
                if report.gitignore_updated {
                    "updated"
                } else {
                    "skipped (already has cupola entries)"
                }
            );
            Ok(())
        }

        Command::Doctor { config } => {
            let loader = TomlConfigLoader;
            let use_case = DoctorUseCase::new(loader);
            let results = use_case.run(&config);

            let mut has_failure = false;
            for result in &results {
                match &result.status {
                    CheckStatus::Ok(msg) => println!("✅ {}: {}", result.name, msg),
                    CheckStatus::Warn(msg) => println!("⚠️  {}: {}", result.name, msg),
                    CheckStatus::Fail(msg) => {
                        println!("❌ {}: {}", result.name, msg);
                        has_failure = true;
                    }
                }
            }

            if has_failure {
                return Err(anyhow::anyhow!("doctor checks failed"));
            }
            Ok(())
        }

        Command::Status => {
            let db_path = Path::new(".cupola/cupola.db");
            if !db_path.exists() {
                println!("No database found. Run `cupola init` first.");
                return Ok(());
            }
            let db = SqliteConnection::open(db_path)?;
            let repo = SqliteIssueRepository::new(db);

            // Load config for max_concurrent_sessions display
            let config_path = Path::new(".cupola/cupola.toml");
            let max_sessions = if config_path.exists() {
                load_toml(config_path).ok().and_then(|t| {
                    let overrides = CliOverrides {
                        polling_interval_secs: None,
                        log_level: None,
                    };
                    let cfg = t.into_config(&overrides);
                    cfg.max_concurrent_sessions
                })
            } else {
                None
            };

            let issues = repo.find_active().await?;
            if issues.is_empty() {
                println!("No active issues.");
            } else {
                let running_count = issues
                    .iter()
                    .filter(|i| i.state.needs_process() && i.current_pid.is_some())
                    .count();

                match max_sessions {
                    Some(max) => println!("Running: {running_count}/{max}"),
                    None => println!("Running: {running_count}"),
                }

                for issue in &issues {
                    let pr_info = match (issue.design_pr_number, issue.impl_pr_number) {
                        (Some(d), Some(i)) => format!("design_pr:#{d} impl_pr:#{i}"),
                        (Some(d), None) => format!("design_pr:#{d}"),
                        (None, Some(i)) => format!("impl_pr:#{i}"),
                        (None, None) => String::new(),
                    };
                    let err_info = issue
                        .error_message
                        .as_deref()
                        .map(|e| format!(" \"{e}\""))
                        .unwrap_or_default();
                    println!(
                        "  #{:<5} {:<30} {:<20} retry:{} {}{}",
                        issue.github_issue_number,
                        format!("{:?}", issue.state),
                        pr_info,
                        issue.retry_count,
                        issue.worktree_path.as_deref().unwrap_or(""),
                        err_info,
                    );
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::adapter::outbound::sqlite_connection::SqliteConnection;
    use crate::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
    use crate::application::port::issue_repository::IssueRepository;
    use crate::domain::issue::Issue;
    use crate::domain::state::State;

    #[tokio::test]
    async fn status_with_no_active_issues() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        let issues = repo.find_active().await.expect("find");
        assert!(issues.is_empty());
    }

    #[tokio::test]
    async fn status_with_active_issues() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        let issue = Issue {
            id: 0,
            github_issue_number: 42,
            state: State::DesignRunning,
            design_pr_number: Some(85),
            impl_pr_number: None,
            worktree_path: Some(".cupola/worktrees/42".into()),
            retry_count: 1,
            current_pid: None,
            error_message: None,
            feature_name: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        repo.save(&issue).await.expect("save");

        let issues = repo.find_active().await.expect("find");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].github_issue_number, 42);
        assert_eq!(issues[0].design_pr_number, Some(85));
    }

    #[test]
    fn init_creates_schema_idempotently() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("first init");
        db.init_schema().expect("second init should also succeed");
    }
}
