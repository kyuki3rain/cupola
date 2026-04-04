use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::adapter::inbound::cli::{Cli, Command, InitAgent};
use crate::adapter::outbound::claude_code_process::ClaudeCodeProcess;
use crate::adapter::outbound::git_worktree_manager::GitWorktreeManager;
use crate::adapter::outbound::github_client_impl::GitHubClientImpl;
use crate::adapter::outbound::github_graphql_client::GraphQLClient;
use crate::adapter::outbound::github_rest_client::OctocrabRestClient;
use crate::adapter::outbound::init_file_generator::InitFileGenerator;
use crate::adapter::outbound::nix_signal_sender::NixSignalSender;
use crate::adapter::outbound::pid_file_manager::PidFileManager;
use crate::adapter::outbound::process_command_runner::ProcessCommandRunner;
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
            let stop_use_case = StopUseCase::with_signal_sender(
                pid_file_manager,
                NixSignalSender,
                Duration::from_secs(30),
            );

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

        Command::Init { agent } => {
            let base_dir = std::env::current_dir().context("failed to get current directory")?;
            let cupola_dir = base_dir.join(".cupola");
            std::fs::create_dir_all(&cupola_dir)
                .with_context(|| format!("failed to create {}", cupola_dir.display()))?;
            let db_path = cupola_dir.join("cupola.db");
            let db_existed = db_path.exists();
            let db = SqliteConnection::open(&db_path).context("failed to open SQLite database")?;
            let file_gen = InitFileGenerator::new(base_dir.clone());
            let runner = ProcessCommandRunner;
            let uc = InitUseCase::new(base_dir, db_existed, db, file_gen, runner, agent.into());
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
                "  agent assets: {}",
                if report.agent_assets_installed {
                    "installed"
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
            println!(
                "  steering bootstrap: {}",
                match report.steering_bootstrap_message {
                    Some(ref msg) => msg,
                    None => "completed",
                }
            );
            println!(
                "  target agent: {}",
                match agent {
                    InitAgent::ClaudeCode => "claude-code",
                }
            );
            Ok(())
        }

        Command::Doctor { config } => {
            let loader = TomlConfigLoader;
            let runner = ProcessCommandRunner;
            let use_case = DoctorUseCase::new(loader, runner);
            let results = use_case.run(&config);

            use crate::application::doctor_use_case::DoctorSection;

            println!("=== Start Readiness ===");
            let mut has_failure = false;
            for result in results.iter().filter(|r| matches!(r.section, DoctorSection::StartReadiness)) {
                match &result.status {
                    CheckStatus::Ok(msg) => println!("✅ {}: {}", result.name, msg),
                    CheckStatus::Warn(msg) => println!("⚠️  {}: {}", result.name, msg),
                    CheckStatus::Fail(msg) => {
                        println!("❌ {}: {}", result.name, msg);
                        has_failure = true;
                    }
                }
                if let Some(fix) = &result.remediation {
                    println!("   fix: {fix}");
                }
            }

            println!();
            println!("=== Operational Readiness ===");
            for result in results.iter().filter(|r| matches!(r.section, DoctorSection::OperationalReadiness)) {
                match &result.status {
                    CheckStatus::Ok(msg) => println!("✅ {}: {}", result.name, msg),
                    CheckStatus::Warn(msg) => println!("⚠️  {}: {}", result.name, msg),
                    CheckStatus::Fail(msg) => println!("❌ {}: {}", result.name, msg),
                }
                if let Some(fix) = &result.remediation {
                    println!("   fix: {fix}");
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

        Command::Compress => {
            let specs_dir = std::path::PathBuf::from(".cupola/specs");
            let uc = crate::application::compress_use_case::CompressUseCase::new(specs_dir);
            let report = uc.find_completed_specs()?;
            if let Some(reason) = report.skipped_reason {
                println!("{reason}");
                return Ok(());
            }
            println!(
                "完了済み spec が {} 件見つかりました。`/cupola:spec-compress` を Claude Code で実行してください。",
                report.completed_count
            );
            Ok(())
        }

        Command::Logs { config, follow } => {
            let toml = load_toml(&config)
                .with_context(|| format!("failed to load config from {}", config.display()))?;
            let overrides = CliOverrides {
                polling_interval_secs: None,
                log_level: None,
            };
            let cfg = toml.into_config(&overrides).context("invalid config")?;

            let log_dir = cfg.log_dir;

            if !log_dir.exists() {
                return Err(anyhow::anyhow!(
                    "Log directory not found: {}",
                    log_dir.display()
                ));
            }

            // Find the latest cupola log file
            let find_latest_log = |dir: &Path| -> Option<std::path::PathBuf> {
                let mut entries: Vec<_> = std::fs::read_dir(dir)
                    .ok()?
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_type().map(|ft| ft.is_file()).unwrap_or(false)
                            && e.file_name()
                                .to_str()
                                .is_some_and(|n| n.starts_with("cupola."))
                    })
                    .collect();
                entries.sort_by_key(|e| e.file_name());
                entries.last().map(|e| e.path())
            };

            let log_path = find_latest_log(&log_dir)
                .ok_or_else(|| anyhow::anyhow!("No log files found in {}", log_dir.display()))?;

            if follow {
                // tail -f equivalent (blocking — runs in dedicated thread)
                use std::io::{BufRead, BufReader, Seek, SeekFrom};

                let log_dir_clone = log_dir.clone();
                tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                    let mut current_path = log_path;
                    let mut file = std::fs::File::open(&current_path)
                        .with_context(|| format!("failed to open {}", current_path.display()))?;
                    file.seek(SeekFrom::End(0))?;
                    let mut reader = BufReader::new(file);
                    let mut line = String::new();

                    loop {
                        match reader.read_line(&mut line) {
                            Ok(0) => {
                                std::thread::sleep(Duration::from_millis(200));

                                // Check for log rotation
                                if let Some(newest) =
                                    find_latest_log(&log_dir_clone).filter(|p| *p != current_path)
                                {
                                    let new_file = std::fs::File::open(&newest)?;
                                    reader = BufReader::new(new_file);
                                    current_path = newest;
                                }
                            }
                            Ok(_) => {
                                print!("{line}");
                                line.clear();
                            }
                            Err(e) => {
                                return Err(anyhow::anyhow!("failed to read log: {e}"));
                            }
                        }
                    }
                })
                .await
                .map_err(|e| anyhow::anyhow!("log follow task failed: {e}"))?
            } else {
                // Show last 20 lines using BufRead (memory efficient)
                use std::collections::VecDeque;
                use std::io::{BufRead, BufReader};

                let file = std::fs::File::open(&log_path)
                    .with_context(|| format!("failed to open {}", log_path.display()))?;
                let reader = BufReader::new(file);
                let mut tail: VecDeque<String> = VecDeque::with_capacity(21);

                for line in reader.lines() {
                    let line =
                        line.with_context(|| format!("failed to read {}", log_path.display()))?;
                    if tail.len() == 20 {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }

                for line in &tail {
                    println!("{line}");
                }
                Ok(())
            }
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
                let overrides = CliOverrides {
                    polling_interval_secs: None,
                    log_level: None,
                };
                match load_toml(config_path).and_then(|t| t.into_config(&overrides)) {
                    Ok(cfg) => cfg.max_concurrent_sessions,
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "設定ファイルの読み込みに失敗しました。一部のステータス情報が表示されない場合があります。"
                        );
                        None
                    }
                }
            } else {
                None
            };

            let pid_file_manager =
                PidFileManager::new(Path::new(".cupola/cupola.pid").to_path_buf());
            handle_status(
                &mut std::io::stdout(),
                &pid_file_manager,
                &repo,
                max_sessions,
            )
            .await
        }
    }
}

/// Returns the PID file path derived from the config file path.
/// Both `start_foreground` and `start_daemon`/`start_daemon_child` use this
/// helper so the path is always `<config_dir>/cupola.pid`.
fn pid_file_path(config: &Path) -> std::path::PathBuf {
    config
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("cupola.pid")
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
    let cfg = toml.into_config(&overrides).context("invalid config")?;
    cfg.validate()
        .map_err(|e| anyhow::anyhow!("config validation failed: {e}"))?;

    // PID file protection (same path as daemon)
    let pid_path = pid_file_path(&config);
    let pid_file_manager = PidFileManager::new(pid_path.clone());

    check_and_clean_pid_file(&pid_file_manager)?;

    // Write our PID before logging initialization (consistent with start_daemon_child)
    use crate::application::port::pid_file::{PidFileError, PidFilePort};
    let my_pid = std::process::id();
    pid_file_manager.write_pid(my_pid).map_err(|e| match e {
        PidFileError::AlreadyExists => {
            println!("cupola is already running");
            std::process::exit(1);
        }
        other => anyhow::anyhow!("failed to write PID file: {other}"),
    })?;

    // Initialize logging (hold guard for app lifetime)
    let _guard = init_logging(cfg.log_level, &cfg.log_dir);

    // Wrap all post-write_pid work in a single async block so that any `?` propagation
    // (tracing, DB open, token resolution, client construction, polling) is captured as a
    // Result rather than causing an early function return. This ensures apply_pid_cleanup
    // is always reached regardless of where the failure occurs.
    let result: anyhow::Result<()> = async {
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

        // Build polling use case with PID file for graceful shutdown cleanup
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
    .await;

    // Fallback cleanup: graceful_shutdown() handles the normal SIGTERM/SIGINT path,
    // but if the async block above returns via any other exit (error, early return, etc.)
    // the PID file may still exist. Delete it here unconditionally; failure is
    // intentionally ignored so it never masks the original outcome.
    apply_pid_cleanup(result, pid_path)
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
    let cfg = toml.into_config(&overrides).context("invalid config")?;
    cfg.validate()
        .map_err(|e| anyhow::anyhow!("config validation failed: {e}"))?;

    let pid_file_manager = PidFileManager::new(pid_file_path(&config));

    // Check for existing running daemon
    check_and_clean_pid_file(&pid_file_manager)?;

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
    let cfg = toml.into_config(&overrides).context("invalid config")?;
    cfg.validate()
        .map_err(|e| anyhow::anyhow!("config validation failed: {e}"))?;

    let config_dir = config
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let pid_path = pid_file_path(&config);
    let pid_file_manager = PidFileManager::new(pid_path.clone());

    // Write our PID
    use crate::application::port::pid_file::{PidFileError, PidFilePort};
    let my_pid = std::process::id();
    pid_file_manager.write_pid(my_pid).map_err(|e| match e {
        PidFileError::AlreadyExists => anyhow::anyhow!("cupola is already running"),
        other => anyhow::anyhow!("failed to write PID file: {other}"),
    })?;

    // Initialize logging to file
    let _guard = init_logging(cfg.log_level, &cfg.log_dir);

    // Wrap all post-write_pid work in a single async block so that any `?` propagation
    // (DB open, token resolution, client construction, polling) is captured as a Result
    // rather than causing an early function return. This ensures apply_pid_cleanup is
    // always reached regardless of where the failure occurs.
    let result: anyhow::Result<()> = async {
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
    .await;

    // Fallback cleanup: graceful_shutdown() handles the normal SIGTERM/SIGINT path,
    // but if the async block above returns via any other exit (error, early return, etc.)
    // the PID file may still exist. Delete it here unconditionally; failure is
    // intentionally ignored so it never masks the original outcome.
    apply_pid_cleanup(result, pid_path)
}

async fn handle_status<W: std::io::Write>(
    out: &mut W,
    pid_mgr: &impl crate::application::port::pid_file::PidFilePort,
    repo: &impl IssueRepository,
    max_sessions: Option<u32>,
) -> Result<()> {
    // Daemon status check (always before issue list)
    match pid_mgr.read_pid() {
        Ok(Some(pid)) => {
            if pid_mgr.is_process_alive(pid) {
                writeln!(out, "Daemon: running (pid={pid})")?;
            } else {
                // Re-read before deleting to guard against TOCTOU: only delete if the
                // stale PID is still present (another daemon may have written a new PID).
                let still_stale = pid_mgr.read_pid().ok().flatten() == Some(pid);
                if still_stale {
                    match pid_mgr.delete_pid() {
                        Ok(()) => writeln!(out, "Daemon: not running (stale PID file cleaned)")?,
                        Err(e) => {
                            tracing::warn!("failed to delete stale PID file: {e}");
                            writeln!(
                                out,
                                "Daemon: not running (stale PID file exists, but cleanup failed)"
                            )?;
                        }
                    }
                } else {
                    writeln!(out, "Daemon: not running")?;
                }
            }
        }
        Ok(None) => writeln!(out, "Daemon: not running")?,
        Err(e) => {
            tracing::warn!("failed to read PID file: {e}");
            writeln!(out, "Daemon: not running")?
        }
    }

    let issues = repo.find_active().await?;
    if issues.is_empty() {
        writeln!(out, "No active issues.")?;
    } else {
        // Cache is_process_alive results to avoid redundant syscalls and keep
        // running_count and pid_info consistent with each other.
        let pid_alive_cache: std::collections::HashMap<u32, bool> = issues
            .iter()
            .filter_map(|i| i.current_pid)
            .collect::<std::collections::HashSet<u32>>()
            .into_iter()
            .map(|pid| (pid, pid_mgr.is_process_alive(pid)))
            .collect();

        let running_count = issues
            .iter()
            .filter(|i| {
                i.state.needs_process()
                    && i.current_pid
                        .is_some_and(|pid| *pid_alive_cache.get(&pid).unwrap_or(&false))
            })
            .count();

        match max_sessions {
            Some(max) => writeln!(out, "Running: {running_count}/{max}")?,
            None => writeln!(out, "Running: {running_count}")?,
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
            let pid_info = match issue.current_pid {
                Some(pid) => {
                    if *pid_alive_cache.get(&pid).unwrap_or(&false) {
                        format!(" pid:{pid} (alive)")
                    } else {
                        format!(" pid:{pid} (dead)")
                    }
                }
                None => String::new(),
            };
            writeln!(
                out,
                "  #{:<5} {:<30} {:<20} retry:{} {}{}{}",
                issue.github_issue_number,
                format!("{:?}", issue.state),
                pr_info,
                issue.retry_count,
                issue.worktree_path.as_deref().unwrap_or(""),
                err_info,
                pid_info,
            )?;
        }
    }
    Ok(())
}

/// Checks the PID file and cleans up stale PIDs.
///
/// - If a PID file exists and the process is alive: returns `Err` (already running).
/// - If a PID file exists but the process is dead: deletes the file and returns `Ok(())`.
/// - If no PID file exists: returns `Ok(())`.
/// - On read error: returns an `Err` that wraps the underlying read error with context.
fn check_and_clean_pid_file(pid_file_manager: &PidFileManager) -> Result<()> {
    use crate::application::port::pid_file::PidFilePort;
    match pid_file_manager.read_pid() {
        Ok(Some(existing_pid)) if pid_file_manager.is_process_alive(existing_pid) => Err(
            anyhow::anyhow!("cupola is already running (pid={existing_pid})"),
        ),
        Ok(Some(_)) => {
            // Stale PID — clean up
            let _ = pid_file_manager.delete_pid();
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("failed to read PID file: {e}")),
    }
}

/// Deletes the PID file at `pid_path` as a best-effort fallback, then returns `result`
/// unchanged. Deletion errors are silently ignored so they never mask the original outcome.
fn apply_pid_cleanup(
    result: anyhow::Result<()>,
    pid_path: std::path::PathBuf,
) -> anyhow::Result<()> {
    use crate::application::port::pid_file::PidFilePort;
    let cleanup = PidFileManager::new(pid_path);
    let _ = cleanup.delete_pid();
    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::adapter::outbound::sqlite_connection::SqliteConnection;
    use crate::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
    use crate::application::port::issue_repository::IssueRepository;
    use crate::application::port::pid_file::{PidFileError, PidFilePort};
    use crate::domain::issue::Issue;
    use crate::domain::state::State;

    // ---------------------------------------------------------------------------
    // Mock PidFilePort
    // ---------------------------------------------------------------------------

    struct MockPidFilePort {
        pid: Option<u32>,
        alive_pids: HashSet<u32>,
        deleted: std::sync::Mutex<bool>,
    }

    impl MockPidFilePort {
        fn new() -> Self {
            Self {
                pid: None,
                alive_pids: HashSet::new(),
                deleted: std::sync::Mutex::new(false),
            }
        }

        fn with_pid(mut self, pid: u32) -> Self {
            self.pid = Some(pid);
            self
        }

        fn with_alive_pid(mut self, pid: u32) -> Self {
            self.alive_pids.insert(pid);
            self
        }

        fn was_deleted(&self) -> bool {
            *self.deleted.lock().unwrap()
        }
    }

    impl PidFilePort for MockPidFilePort {
        fn write_pid(&self, _pid: u32) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_pid(&self) -> Result<Option<u32>, PidFileError> {
            Ok(self.pid)
        }

        fn delete_pid(&self) -> Result<(), PidFileError> {
            *self.deleted.lock().unwrap() = true;
            Ok(())
        }

        fn is_process_alive(&self, pid: u32) -> bool {
            self.alive_pids.contains(&pid)
        }
    }

    fn make_issue(issue_number: u64, state: State, current_pid: Option<u32>) -> Issue {
        Issue {
            id: 0,
            github_issue_number: issue_number,
            state,
            design_pr_number: None,
            impl_pr_number: None,
            worktree_path: None,
            retry_count: 0,
            ci_fix_count: 0,
            current_pid,
            error_message: None,
            feature_name: format!("issue-{issue_number}"),
            fixing_causes: vec![],
            weight: crate::domain::task_weight::TaskWeight::Medium,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

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
            ci_fix_count: 0,
            current_pid: None,
            error_message: None,
            feature_name: "issue-42".to_string(),
            fixing_causes: vec![],
            weight: crate::domain::task_weight::TaskWeight::Medium,
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

    // --- apply_pid_cleanup tests ---

    #[test]
    fn apply_pid_cleanup_deletes_pid_file_on_ok_result() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("cupola.pid");
        std::fs::write(&path, "12345\n").expect("write");
        assert!(path.exists());

        let result = super::apply_pid_cleanup(Ok(()), path.clone());

        assert!(result.is_ok());
        assert!(!path.exists(), "PID file should be deleted after Ok result");
    }

    #[test]
    fn apply_pid_cleanup_deletes_pid_file_on_err_result() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("cupola.pid");
        std::fs::write(&path, "12345\n").expect("write");
        assert!(path.exists());

        let original_err = anyhow::anyhow!("polling failed");
        let result = super::apply_pid_cleanup(Err(original_err), path.clone());

        assert!(result.is_err(), "original Err should be preserved");
        assert!(
            !path.exists(),
            "PID file should be deleted even after Err result"
        );
    }

    #[test]
    fn apply_pid_cleanup_preserves_original_err_when_deletion_fails() {
        // Create a directory at the PID path so remove_file() fails deterministically
        // (you cannot remove_file() a directory on most platforms).
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("cupola.pid");
        std::fs::create_dir(&path).expect("create directory at pid path");
        assert!(path.is_dir(), "test setup: pid path must be a directory");

        let original_err = anyhow::anyhow!("some polling error");
        let err_msg = original_err.to_string();
        let result = super::apply_pid_cleanup(Err(original_err), path);

        // The function must return the original Err, not the deletion error.
        let err = result.expect_err("should return Err");
        assert_eq!(err.to_string(), err_msg, "original error message preserved");
    }

    #[test]
    fn apply_pid_cleanup_ok_when_pid_file_absent() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("cupola.pid");
        // File does not exist

        let result = super::apply_pid_cleanup(Ok(()), path.clone());

        assert!(
            result.is_ok(),
            "Ok(()) should pass through even when PID file absent"
        );
        assert!(!path.exists());
    }

    // --- handle_status: Task 3.1 daemon status display tests ---

    #[tokio::test]
    async fn daemon_status_running_when_pid_is_alive() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        let pid_mgr = MockPidFilePort::new().with_pid(1234).with_alive_pid(1234);

        let mut out = Vec::new();
        super::handle_status(&mut out, &pid_mgr, &repo, None)
            .await
            .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(
            output.contains("Daemon: running (pid=1234)"),
            "output: {output}"
        );
    }

    #[tokio::test]
    async fn daemon_status_not_running_when_no_pid_file() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(&mut out, &pid_mgr, &repo, None)
            .await
            .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("Daemon: not running"), "output: {output}");
    }

    #[tokio::test]
    async fn daemon_status_stale_pid_cleaned() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        // PID in file but process is not alive
        let pid_mgr = MockPidFilePort::new().with_pid(5678);

        let mut out = Vec::new();
        super::handle_status(&mut out, &pid_mgr, &repo, None)
            .await
            .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(pid_mgr.was_deleted(), "PID file should be deleted");
        assert!(
            output.contains("Daemon: not running (stale PID file cleaned)"),
            "output: {output}"
        );
    }

    #[tokio::test]
    async fn daemon_status_appears_before_issue_list() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        repo.save(&make_issue(42, State::DesignRunning, None))
            .await
            .expect("save");

        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(&mut out, &pid_mgr, &repo, None)
            .await
            .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        let daemon_pos = output.find("Daemon:").expect("should have daemon line");
        let issue_pos = output.find("#42").expect("should have issue line");
        assert!(
            daemon_pos < issue_pos,
            "Daemon status should appear before issue list; output: {output}"
        );
    }

    // --- handle_status: Task 3.2 issue process alive/dead tests ---

    #[tokio::test]
    async fn issue_with_alive_pid_shows_alive() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        repo.save(&make_issue(10, State::DesignRunning, Some(100)))
            .await
            .expect("save");

        let pid_mgr = MockPidFilePort::new().with_alive_pid(100);

        let mut out = Vec::new();
        super::handle_status(&mut out, &pid_mgr, &repo, None)
            .await
            .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("pid:100 (alive)"), "output: {output}");
    }

    #[tokio::test]
    async fn issue_with_dead_pid_shows_dead() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        repo.save(&make_issue(11, State::DesignRunning, Some(200)))
            .await
            .expect("save");

        // 200 is not in alive_pids → dead
        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(&mut out, &pid_mgr, &repo, None)
            .await
            .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("pid:200 (dead)"), "output: {output}");
    }

    #[tokio::test]
    async fn issue_with_no_pid_shows_no_pid_info() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        repo.save(&make_issue(12, State::DesignRunning, None))
            .await
            .expect("save");

        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(&mut out, &pid_mgr, &repo, None)
            .await
            .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        let issue_line = output
            .lines()
            .find(|l| l.contains("#12"))
            .expect("should have issue line");
        assert!(
            !issue_line.contains("pid:"),
            "should not contain pid info; line: {issue_line}"
        );
    }

    #[tokio::test]
    async fn running_count_only_counts_alive_processes() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        // Two issues with PIDs, only pid 300 is alive
        repo.save(&make_issue(20, State::DesignRunning, Some(300)))
            .await
            .expect("save");
        repo.save(&make_issue(21, State::DesignRunning, Some(301)))
            .await
            .expect("save");

        let pid_mgr = MockPidFilePort::new().with_alive_pid(300);

        let mut out = Vec::new();
        super::handle_status(&mut out, &pid_mgr, &repo, None)
            .await
            .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(
            output.contains("Running: 1"),
            "expected Running: 1; output: {output}"
        );
    }

    // --- check_and_clean_pid_file tests ---

    use crate::adapter::outbound::pid_file_manager::PidFileManager;

    #[test]
    fn check_and_clean_pid_file_returns_err_when_process_alive() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("cupola.pid");
        let mgr = PidFileManager::new(path.clone());

        // Write current process PID so the process is definitely alive
        let my_pid = std::process::id();
        mgr.write_pid(my_pid).expect("write");

        let result = super::check_and_clean_pid_file(&mgr);

        assert!(result.is_err(), "should return Err when process is alive");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("already running"),
            "error should mention 'already running', got: {msg}"
        );
        assert!(
            msg.contains(&my_pid.to_string()),
            "error should contain the PID, got: {msg}"
        );
        // PID file should NOT be deleted
        assert!(
            path.exists(),
            "PID file should not be deleted when process alive"
        );
    }

    #[test]
    fn check_and_clean_pid_file_removes_stale_pid_and_returns_ok() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("cupola.pid");
        let mgr = PidFileManager::new(path.clone());

        // Find a PID in a broad range that this environment currently considers dead,
        // so the test does not silently skip based on real OS process state for a
        // single hard-coded PID.
        let stale_pid = (999_999..1_010_000)
            .find(|pid| !mgr.is_process_alive(*pid))
            .expect("expected to find at least one dead PID in test range");
        std::fs::write(&path, format!("{stale_pid}\n")).expect("write");

        let result = super::check_and_clean_pid_file(&mgr);

        assert!(result.is_ok(), "should return Ok(()) for stale PID");
        assert!(!path.exists(), "stale PID file should be deleted");
    }

    #[test]
    fn check_and_clean_pid_file_returns_ok_when_no_pid_file() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("cupola.pid");
        let mgr = PidFileManager::new(path);
        // File does not exist

        let result = super::check_and_clean_pid_file(&mgr);

        assert!(
            result.is_ok(),
            "should return Ok(()) when no PID file exists"
        );
    }

    #[test]
    fn check_and_clean_pid_file_returns_err_on_invalid_content() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("cupola.pid");
        // Write invalid content (not a number)
        std::fs::write(&path, "not-a-pid\n").expect("write");
        let mgr = PidFileManager::new(path);

        let result = super::check_and_clean_pid_file(&mgr);

        assert!(
            result.is_err(),
            "should return Err for invalid PID file content"
        );
    }

    #[test]
    fn pid_file_path_helper_uses_config_dir() {
        // Verify that pid_file_path() returns <config_dir>/cupola.pid.
        // Both start_foreground and start_daemon/start_daemon_child delegate to this
        // helper, so testing it once covers all callers.
        let dir = tempfile::TempDir::new().expect("temp dir");
        let config_path = dir.path().join("cupola.toml");
        let expected_pid_path = dir.path().join("cupola.pid");

        let pid_path = super::pid_file_path(&config_path);

        assert_eq!(
            pid_path, expected_pid_path,
            "PID file path should be <config_dir>/cupola.pid"
        );
    }
}
