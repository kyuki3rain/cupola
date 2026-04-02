use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::adapter::inbound::cli::{Cli, Command};
use crate::adapter::outbound::claude_code_process::ClaudeCodeProcess;
use crate::adapter::outbound::git_worktree_manager::GitWorktreeManager;
use crate::adapter::outbound::github_client_impl::GitHubClientImpl;
use crate::adapter::outbound::github_graphql_client::GraphQLClient;
use crate::adapter::outbound::github_rest_client::OctocrabRestClient;
use crate::adapter::outbound::pid_file_manager::PidFileManager;
use crate::adapter::outbound::sqlite_connection::SqliteConnection;
use crate::adapter::outbound::sqlite_execution_log_repository::SqliteExecutionLogRepository;
use crate::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
use crate::application::cleanup_use_case::CleanupUseCase;
use crate::application::doctor_use_case::{CheckStatus, DoctorUseCase};
use crate::application::init_use_case::InitUseCase;
use crate::application::polling_use_case::PollingUseCase;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::stop_use_case::StopUseCase;
use crate::bootstrap::config_loader::{CliOverrides, load_toml};
use crate::bootstrap::logging::init_logging;
use crate::bootstrap::toml_config_loader::TomlConfigLoader;

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Start {
            polling_interval_secs,
            log_level,
            config,
            daemon,
            daemon_child,
        } => {
            if daemon_child {
                start_daemon_child(config, polling_interval_secs, log_level).await
            } else if daemon {
                start_daemon(config, polling_interval_secs, log_level).await
            } else {
                start_foreground(config, polling_interval_secs, log_level).await
            }
        }

        Command::Stop { config } => {
            let config_dir = config
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            let pid_file_manager = PidFileManager::new(config_dir.join("cupola.pid"));
            let stop_use_case = StopUseCase::new(pid_file_manager, Duration::from_secs(30));

            match stop_use_case.execute().await? {
                crate::application::stop_use_case::StopResult::NotRunning => {
                    println!("cupola is not running");
                }
                crate::application::stop_use_case::StopResult::StalePidCleaned { pid: _ } => {
                    println!("cleaned up stale PID file");
                }
                crate::application::stop_use_case::StopResult::Stopped { pid } => {
                    println!("stopped cupola (pid={pid})");
                }
                crate::application::stop_use_case::StopResult::ForceKilled { pid } => {
                    println!("force killed cupola (pid={pid})");
                }
            }
            Ok(())
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

        Command::Cleanup { config } => {
            let db_path = config
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("cupola.db");
            if !db_path.exists() {
                println!("No database found. Run `cupola init` first.");
                return Ok(());
            }
            let db = SqliteConnection::open(&db_path)?;
            let issue_repo = SqliteIssueRepository::new(db);
            let worktree = GitWorktreeManager::new(".");
            let uc = CleanupUseCase::new(issue_repo, worktree);
            println!("⚠️  daemon が動作中の場合は停止してから cleanup を実行してください");
            let result = uc.execute().await?;
            if result.cleaned.is_empty() {
                println!("対象の Cancelled Issue が見つかりませんでした");
            } else {
                println!(
                    "cleanup 完了: {} 件の Issue を処理しました",
                    result.cleaned.len()
                );
                for item in &result.cleaned {
                    println!("  Issue #{}", item.issue_number);
                    if item.worktree_removed {
                        println!("    ✓ worktree 削除済み");
                    }
                    for branch in &item.branches_removed {
                        println!("    ✓ ブランチ削除: {branch}");
                    }
                }
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

async fn start_foreground(
    config: std::path::PathBuf,
    polling_interval_secs: Option<u64>,
    log_level: Option<String>,
) -> Result<()> {
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

    // Build and run polling use case (no PID file in foreground mode)
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

/// Launch the daemon by spawning a fresh child process (re-exec).
/// This avoids calling fork() inside the Tokio async runtime, which can
/// cause deadlocks due to inherited thread/lock state.
async fn start_daemon(
    config: std::path::PathBuf,
    polling_interval_secs: Option<u64>,
    log_level: Option<String>,
) -> Result<()> {
    let toml = load_toml(&config)
        .with_context(|| format!("failed to load config from {}", config.display()))?;

    let overrides = CliOverrides {
        polling_interval_secs,
        log_level: log_level.clone(),
    };
    let cfg = toml.into_config(&overrides);
    cfg.validate()
        .map_err(|e| anyhow::anyhow!("config validation failed: {e}"))?;

    // log_dir must be set for daemon mode (no terminal to log to)
    if cfg.log_dir.is_none() {
        return Err(anyhow::anyhow!(
            "daemon mode requires [log] dir to be set in cupola.toml"
        ));
    }

    let config_dir = config
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let pid_file_manager = PidFileManager::new(config_dir.join("cupola.pid"));

    // Check for existing running daemon
    use crate::application::port::pid_file::PidFilePort;
    match pid_file_manager.read_pid() {
        Ok(Some(existing_pid)) if pid_file_manager.is_process_alive(existing_pid) => {
            return Err(anyhow::anyhow!(
                "cupola daemon is already running (pid={existing_pid})"
            ));
        }
        Ok(Some(_)) => {
            // Stale PID — clean up
            let _ = pid_file_manager.delete_pid();
        }
        Ok(None) => {}
        Err(e) => {
            return Err(anyhow::anyhow!("failed to read PID file: {e}"));
        }
    }

    // Spawn a fresh child process (re-exec self with --daemon-child flag).
    // This ensures the Tokio runtime in the child starts clean with no
    // inherited thread or lock state from the parent.
    let exe = std::env::current_exe().context("failed to get current executable path")?;
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("start")
        .arg("--daemon-child")
        .arg("--config")
        .arg(&config);
    if let Some(secs) = polling_interval_secs {
        cmd.arg("--polling-interval-secs").arg(secs.to_string());
    }
    if let Some(ref level) = log_level {
        cmd.arg("--log-level").arg(level);
    }
    // Redirect all stdio to /dev/null so the daemon is fully detached.
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let child = cmd
        .spawn()
        .context("failed to spawn daemon child process")?;
    let child_pid = child.id();
    // Drop the Child handle — Rust does not kill the child on drop,
    // so the daemon continues running independently.
    drop(child);

    println!("started cupola daemon (pid={child_pid})");
    Ok(())
}

/// Run as the daemon child process. Called when --daemon-child is passed.
async fn start_daemon_child(
    config: std::path::PathBuf,
    polling_interval_secs: Option<u64>,
    log_level: Option<String>,
) -> Result<()> {
    // Detach from the parent's session so the daemon survives terminal closure.
    nix::unistd::setsid().context("setsid failed")?;

    let toml = load_toml(&config)
        .with_context(|| format!("failed to load config from {}", config.display()))?;

    let overrides = CliOverrides {
        polling_interval_secs,
        log_level,
    };
    let cfg = toml.into_config(&overrides);
    cfg.validate()
        .map_err(|e| anyhow::anyhow!("config validation failed: {e}"))?;

    let config_dir = config
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let pid_file_manager = PidFileManager::new(config_dir.join("cupola.pid"));

    // Write our PID
    use crate::application::port::pid_file::PidFilePort;
    let my_pid = std::process::id();
    pid_file_manager
        .write_pid(my_pid)
        .map_err(|e| anyhow::anyhow!("failed to write PID file: {e}"))?;

    // Initialize logging to file
    let _guard = init_logging(cfg.log_level, cfg.log_dir.as_deref());

    tracing::info!(
        owner = %cfg.owner,
        repo = %cfg.repo,
        pid = my_pid,
        "starting cupola daemon"
    );

    // Initialize SQLite
    let db_path = config_dir.join("cupola.db");
    let db = SqliteConnection::open(&db_path)?;
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

    // Build polling use case with PID file
    let mut polling = PollingUseCase::new(
        github,
        issue_repo,
        exec_log_repo,
        claude_runner,
        worktree,
        cfg,
    )
    .with_pid_file(Box::new(pid_file_manager));

    polling.run().await
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
            fixing_causes: vec![],
            model: None,
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
