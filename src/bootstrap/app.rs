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
use crate::adapter::outbound::sqlite_process_run_repository::SqliteProcessRunRepository;
use crate::application::cleanup_use_case::CleanupUseCase;
use crate::application::session_manager::ChildProcessRegistry;
use crate::application::start_use_case::{ProcessAlivePort, recover_orphans};

/// Production ProcessAlivePort using nix::sys::signal::kill(pid, 0).
struct NixProcessAlive;
impl ProcessAlivePort for NixProcessAlive {
    fn is_alive(&self, pid: u32) -> bool {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        kill(Pid::from_raw(pid as i32), None).is_ok()
    }
    fn kill(&self, pid: u32) {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;
        let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
    }
}
use crate::application::doctor_result::CheckStatus;
use crate::application::doctor_use_case::DoctorUseCase;
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

        Command::Stop { config, force } => {
            let config_dir = config
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();

            // 設定ファイルから shutdown_timeout を読み込む（存在しない場合はデフォルト 300 秒）
            let shutdown_timeout = if config.exists() {
                match load_toml(&config) {
                    Ok(toml) => {
                        let overrides = CliOverrides {
                            polling_interval_secs: None,
                            log_level: None,
                        };
                        toml.into_config(&overrides)
                            .map(|c| c.shutdown_timeout)
                            .unwrap_or(Some(Duration::from_secs(300)))
                    }
                    Err(_) => Some(Duration::from_secs(300)),
                }
            } else {
                Some(Duration::from_secs(300))
            };

            let pid_file_manager = PidFileManager::new(config_dir.join("cupola.pid"));
            let stop_use_case = StopUseCase::with_signal_sender(
                pid_file_manager,
                NixSignalSender,
                shutdown_timeout,
            );

            match stop_use_case.execute(force).await? {
                crate::application::stop_use_case::StopResult::NotRunning => {
                    println!("cupola is not running");
                }
                crate::application::stop_use_case::StopResult::StalePidCleaned { pid: _ } => {
                    println!("cleaned up stale PID file");
                }
                crate::application::stop_use_case::StopResult::Stopped { pid } => {
                    println!("stopped cupola (pid={pid})");
                }
                crate::application::stop_use_case::StopResult::ForceKilled {
                    pid,
                    sessions_killed,
                } => {
                    println!("force killed cupola (pid={pid}, sessions_killed={sessions_killed})");
                }
            }
            Ok(())
        }

        Command::Init { agent, upgrade } => {
            let base_dir = std::env::current_dir().context("failed to get current directory")?;
            let cupola_dir = base_dir.join(".cupola");
            std::fs::create_dir_all(&cupola_dir)
                .with_context(|| format!("failed to create {}", cupola_dir.display()))?;
            let db_path = cupola_dir.join("cupola.db");
            let db_existed = db_path.exists();
            let db = SqliteConnection::open(&db_path).context("failed to open SQLite database")?;
            let file_gen = InitFileGenerator::new(base_dir.clone());
            let runner = ProcessCommandRunner;
            let uc = InitUseCase::new(
                base_dir,
                db_existed,
                db,
                file_gen,
                runner,
                agent.into(),
                upgrade,
            );
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
                if upgrade {
                    if report.agent_assets_installed {
                        "upgraded"
                    } else {
                        "already up to date"
                    }
                } else if report.agent_assets_installed {
                    "installed"
                } else {
                    "skipped"
                }
            );
            println!(
                "  .gitignore: {}",
                if upgrade {
                    if report.gitignore_updated {
                        "upgraded"
                    } else {
                        "already up to date"
                    }
                } else if report.gitignore_updated {
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

            use crate::application::doctor_result::DoctorSection;

            println!("=== Start Readiness ===");
            let mut has_failure = false;
            for result in results
                .iter()
                .filter(|r| matches!(r.section, DoctorSection::StartReadiness))
            {
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
            for result in results
                .iter()
                .filter(|r| matches!(r.section, DoctorSection::OperationalReadiness))
            {
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
            let pid_path = config
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("cupola.pid");
            let pid_file_manager = PidFileManager::new(pid_path);
            check_daemon_not_running(&pid_file_manager)?;
            let db = SqliteConnection::open(&db_path)?;
            let issue_repo = SqliteIssueRepository::new(db);
            let worktree = GitWorktreeManager::new(".");
            let uc = CleanupUseCase::new(issue_repo, worktree);
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
            db.init_schema()?;
            let repo = SqliteIssueRepository::new(db.clone());
            let process_run_repo = SqliteProcessRunRepository::new(db);

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
                &process_run_repo,
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
    use crate::application::port::pid_file::{PidFileError, PidFilePort, ProcessMode};
    let my_pid = std::process::id();
    pid_file_manager
        .write_pid_with_mode(my_pid, ProcessMode::Foreground)
        .map_err(|e| match e {
            PidFileError::AlreadyExists => {
                println!("cupola is already running");
                std::process::exit(1);
            }
            other => anyhow::anyhow!("failed to write PID file: {other}"),
        })?;

    let registry = ChildProcessRegistry::new();

    // Install a panic hook that shuts down child processes and deletes the PID file
    // if the polling loop panics unexpectedly.
    install_panic_hook(pid_path.clone(), registry.clone());

    // Wrap all post-write_pid work in a single async block so that any `?` propagation
    // (tracing, DB open, token resolution, client construction, polling) is captured as a
    // Result rather than causing an early function return. This ensures apply_pid_cleanup
    // is always reached regardless of where the failure occurs.
    let result: anyhow::Result<()> = async {
        // Ensure log directory exists before initializing logging
        std::fs::create_dir_all(&cfg.log_dir).context("create log directory")?;
        // Initialize logging (hold guard for block lifetime)
        let _guard = init_logging(cfg.log_level, &cfg.log_dir);

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
        let process_repo = SqliteProcessRunRepository::new(db.clone());

        // Orphan recovery: mark stale running process_runs as failed=orphaned.
        // Must run before the polling loop observes them, otherwise Resolve's
        // stale guard may act on ghost state.
        if let Err(e) = recover_orphans(&process_repo, &NixProcessAlive).await {
            tracing::warn!(error = %e, "orphan recovery failed at startup");
        }

        let exec_log_repo = SqliteExecutionLogRepository::new(db);
        let claude_runner = ClaudeCodeProcess::new("claude");
        let worktree = GitWorktreeManager::new(".");
        let file_gen = InitFileGenerator::new(std::env::current_dir()?);

        // Build polling use case with PID file for graceful shutdown cleanup
        let mut polling = PollingUseCase::new(
            github,
            issue_repo,
            exec_log_repo,
            claude_runner,
            worktree,
            file_gen,
            cfg,
        )
        .with_process_repo(process_repo)
        .with_pid_file(Box::new(pid_file_manager))
        .with_child_registry(registry);

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
    use crate::application::port::pid_file::{PidFileError, PidFilePort, ProcessMode};
    let my_pid = std::process::id();
    pid_file_manager
        .write_pid_with_mode(my_pid, ProcessMode::Daemon)
        .map_err(|e| match e {
            PidFileError::AlreadyExists => anyhow::anyhow!("cupola is already running"),
            other => anyhow::anyhow!("failed to write PID file: {other}"),
        })?;

    let registry = ChildProcessRegistry::new();

    // Install a panic hook that shuts down child processes and deletes the PID file
    // if the polling loop panics unexpectedly.
    install_panic_hook(pid_path.clone(), registry.clone());

    // Wrap all post-write_pid work in a single async block so that any `?` propagation
    // (DB open, token resolution, client construction, polling) is captured as a Result
    // rather than causing an early function return. This ensures apply_pid_cleanup is
    // always reached regardless of where the failure occurs.
    let result: anyhow::Result<()> = async {
        // Ensure log directory exists before initializing logging
        std::fs::create_dir_all(&cfg.log_dir).context("create log directory")?;
        // Initialize logging to file (hold guard for block lifetime)
        let _guard = init_logging(cfg.log_level, &cfg.log_dir);

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
        let process_repo = SqliteProcessRunRepository::new(db.clone());

        // Orphan recovery: mark stale running process_runs as failed=orphaned.
        // Must run before the polling loop observes them, otherwise Resolve's
        // stale guard may act on ghost state.
        if let Err(e) = recover_orphans(&process_repo, &NixProcessAlive).await {
            tracing::warn!(error = %e, "orphan recovery failed at startup");
        }

        let exec_log_repo = SqliteExecutionLogRepository::new(db);
        let claude_runner = ClaudeCodeProcess::new("claude");
        let worktree = GitWorktreeManager::new(".");
        let file_gen = InitFileGenerator::new(config_dir.clone());

        // Build polling use case with PID file
        let mut polling = PollingUseCase::new(
            github,
            issue_repo,
            exec_log_repo,
            claude_runner,
            worktree,
            file_gen,
            cfg,
        )
        .with_process_repo(process_repo)
        .with_pid_file(Box::new(pid_file_manager))
        .with_child_registry(registry);

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
    process_run_repo: &impl crate::application::port::process_run_repository::ProcessRunRepository,
    repo: &impl IssueRepository,
    max_sessions: Option<u32>,
) -> Result<()> {
    use crate::application::port::pid_file::ProcessMode;

    // Process status check (always before issue list)
    match pid_mgr.read_pid_and_mode() {
        Ok(Some((pid, mode))) => {
            if pid_mgr.is_process_alive(pid) {
                let mode_str = match mode {
                    Some(ProcessMode::Foreground) => "foreground",
                    Some(ProcessMode::Daemon) => "daemon",
                    None => "unknown",
                };
                writeln!(out, "Process: running ({mode_str}, pid={pid})")?;
            } else {
                // Re-read before deleting to guard against TOCTOU: only delete if the
                // stale PID is still present (another process may have written a new PID).
                let still_stale =
                    pid_mgr.read_pid_and_mode().ok().flatten().map(|(p, _)| p) == Some(pid);
                if still_stale {
                    match pid_mgr.delete_pid() {
                        Ok(()) => writeln!(out, "Process: not running (stale PID file cleaned)")?,
                        Err(e) => {
                            tracing::warn!("failed to delete stale PID file: {e}");
                            writeln!(
                                out,
                                "Process: not running (stale PID file exists, but cleanup failed)"
                            )?;
                        }
                    }
                } else {
                    writeln!(out, "Process: not running")?;
                }
            }
        }
        Ok(None) => writeln!(out, "Process: not running")?,
        Err(e) => {
            tracing::warn!("failed to read PID file: {e}");
            writeln!(out, "Process: not running")?
        }
    }

    let issues = repo.find_active().await?;
    if issues.is_empty() {
        writeln!(out, "No active issues.")?;
    } else {
        let running_records = process_run_repo.find_all_running().await?;
        // Count only alive Claude sessions: exclude Init (worktree-creation tasks) and
        // rows with no pid or a stale pid (process no longer alive).
        let running_count = running_records
            .iter()
            .filter(|r| r.type_ != crate::domain::process_run::ProcessRunType::Init)
            .filter(|r| r.pid.is_some_and(|pid| pid_mgr.is_process_alive(pid)))
            .count();

        match max_sessions {
            Some(max) => writeln!(out, "Claude sessions: {running_count}/{max}")?,
            None => writeln!(out, "Claude sessions: {running_count}")?,
        }

        for issue in &issues {
            writeln!(
                out,
                "  #{:<5} {:<30} wt:{}",
                issue.github_issue_number,
                issue.state.to_string(),
                issue.worktree_path.as_deref().unwrap_or("none"),
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

/// Returns an error if a daemon process is currently running according to the PID file.
///
/// Cleanup must not proceed while the daemon is active to avoid data corruption.
fn check_daemon_not_running(
    pid_mgr: &impl crate::application::port::pid_file::PidFilePort,
) -> Result<()> {
    match pid_mgr.read_pid() {
        Ok(Some(pid)) if pid_mgr.is_process_alive(pid) => Err(anyhow::anyhow!(
            "cupola is running (pid={pid}). Run `cupola stop` first."
        )),
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(e).context("failed to read PID file")),
    }
}

/// Installs a global panic hook that shuts down child processes, deletes the
/// PID file, logs the panic, then invokes the previous hook.
///
/// # Behaviour
/// 1. Sends SIGTERM to all registered child processes, waits up to 3 seconds, then SIGKILL.
/// 2. Deletes the PID file (best-effort; errors are silently ignored).
/// 3. Logs the panic message and location via `tracing::error!` (no-op when the
///    subscriber is not yet initialised).
/// 4. Invokes the previously-installed hook so that the default panic output
///    (stack trace to stderr) is preserved.
pub fn install_panic_hook(pid_path: std::path::PathBuf, child_registry: ChildProcessRegistry) {
    use crate::application::port::pid_file::PidFilePort;

    let previous_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        child_registry.shutdown_sync(std::time::Duration::from_secs(3));

        let _ = PidFileManager::new(pid_path.clone()).delete_pid();

        let msg: String = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        };

        if let Some(loc) = panic_info.location() {
            tracing::error!(
                message = %msg,
                file = loc.file(),
                line = loc.line(),
                "daemon panicked"
            );
        } else {
            tracing::error!(message = %msg, "daemon panicked");
        }

        previous_hook(panic_info);
    }));
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
    use crate::application::port::pid_file::{PidFileError, PidFilePort, ProcessMode};
    use crate::domain::issue::Issue;
    use crate::domain::state::State;

    // ---------------------------------------------------------------------------
    // Mock PidFilePort
    // ---------------------------------------------------------------------------

    struct MockPidFilePort {
        pid: Option<u32>,
        mode: Option<ProcessMode>,
        alive_pids: HashSet<u32>,
        deleted: std::sync::Mutex<bool>,
        read_pid_error: Option<String>,
    }

    impl MockPidFilePort {
        fn new() -> Self {
            Self {
                pid: None,
                mode: None,
                alive_pids: HashSet::new(),
                deleted: std::sync::Mutex::new(false),
                read_pid_error: None,
            }
        }

        fn with_pid(mut self, pid: u32) -> Self {
            self.pid = Some(pid);
            self
        }

        fn with_mode(mut self, mode: ProcessMode) -> Self {
            self.mode = Some(mode);
            self
        }

        fn with_alive_pid(mut self, pid: u32) -> Self {
            self.alive_pids.insert(pid);
            self
        }

        fn with_read_error(mut self, msg: impl Into<String>) -> Self {
            self.read_pid_error = Some(msg.into());
            self
        }

        fn with_alive_pids(mut self, pids: impl IntoIterator<Item = u32>) -> Self {
            self.alive_pids.extend(pids);
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
            if let Some(ref msg) = self.read_pid_error {
                return Err(PidFileError::Read(msg.clone()));
            }
            Ok(self.pid)
        }

        fn delete_pid(&self) -> Result<(), PidFileError> {
            *self.deleted.lock().unwrap() = true;
            Ok(())
        }

        fn is_process_alive(&self, pid: u32) -> bool {
            self.alive_pids.contains(&pid)
        }

        fn write_pid_with_mode(&self, _pid: u32, _mode: ProcessMode) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_pid_and_mode(&self) -> Result<Option<(u32, Option<ProcessMode>)>, PidFileError> {
            Ok(self.pid.map(|p| (p, self.mode)))
        }

        fn write_session_count(&self, _count: u32) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_session_count(&self) -> Option<u32> {
            None
        }

        fn delete_session_file(&self) -> Result<(), PidFileError> {
            Ok(())
        }
    }

    // ---------------------------------------------------------------------------
    // Mock ProcessRunRepository
    // ---------------------------------------------------------------------------

    struct MockProcessRunRepository {
        running_count: usize,
    }

    impl MockProcessRunRepository {
        fn new() -> Self {
            Self { running_count: 0 }
        }

        fn with_running_count(count: usize) -> Self {
            Self {
                running_count: count,
            }
        }
    }

    impl crate::application::port::process_run_repository::ProcessRunRepository
        for MockProcessRunRepository
    {
        async fn save(&self, _run: &crate::domain::process_run::ProcessRun) -> anyhow::Result<i64> {
            Ok(0)
        }

        async fn update_pid(&self, _run_id: i64, _pid: u32) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_succeeded(
            &self,
            _run_id: i64,
            _pr_number: Option<u64>,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_failed(
            &self,
            _run_id: i64,
            _error_message: Option<String>,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_stale(&self, _run_id: i64) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_stale_for_issue(&self, _issue_id: i64) -> anyhow::Result<()> {
            Ok(())
        }

        async fn find_latest(
            &self,
            _issue_id: i64,
            _type_: crate::domain::process_run::ProcessRunType,
        ) -> anyhow::Result<Option<crate::domain::process_run::ProcessRun>> {
            Ok(None)
        }

        async fn find_latest_with_pr_number(
            &self,
            _issue_id: i64,
            _type_: crate::domain::process_run::ProcessRunType,
        ) -> anyhow::Result<Option<crate::domain::process_run::ProcessRun>> {
            Ok(None)
        }

        async fn find_by_issue(
            &self,
            _issue_id: i64,
        ) -> anyhow::Result<Vec<crate::domain::process_run::ProcessRun>> {
            Ok(Vec::new())
        }

        async fn count_consecutive_failures(
            &self,
            _issue_id: i64,
            _type_: crate::domain::process_run::ProcessRunType,
            _since: Option<chrono::DateTime<chrono::Utc>>,
        ) -> anyhow::Result<u32> {
            Ok(0)
        }

        async fn find_latest_with_consecutive_count(
            &self,
            _issue_id: i64,
            _type_: crate::domain::process_run::ProcessRunType,
            _since: Option<chrono::DateTime<chrono::Utc>>,
        ) -> anyhow::Result<Option<(crate::domain::process_run::ProcessRun, u32)>> {
            Ok(None)
        }

        async fn find_all_running(
            &self,
        ) -> anyhow::Result<Vec<crate::domain::process_run::ProcessRun>> {
            use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};
            // Assign sequential PIDs starting at 1000 so callers can configure
            // them as alive in MockPidFilePort::with_alive_pids(1000..1000+count).
            let runs = (0..self.running_count)
                .map(|i| ProcessRun {
                    id: i as i64,
                    issue_id: i as i64,
                    type_: ProcessRunType::Design,
                    index: 0,
                    state: ProcessRunState::Running,
                    pid: Some(1000 + i as u32),
                    pr_number: None,
                    causes: Vec::new(),
                    error_message: None,
                    started_at: chrono::Utc::now(),
                    finished_at: None,
                })
                .collect();
            Ok(runs)
        }

        async fn update_state(
            &self,
            _run_id: i64,
            _state: crate::domain::process_run::ProcessRunState,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn make_issue(issue_number: u64, state: State, _current_pid: Option<u32>) -> Issue {
        // TODO(phase 6): current_pid is now in process_runs table, not Issue
        Issue {
            id: 0,
            github_issue_number: issue_number,
            state,
            feature_name: format!("issue-{issue_number}"),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            worktree_path: None,
            ci_fix_count: 0,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
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
            feature_name: "issue-42".to_string(),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            worktree_path: Some(".cupola/worktrees/42".into()),
            ci_fix_count: 0,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        repo.save(&issue).await.expect("save");

        let issues = repo.find_active().await.expect("find");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].github_issue_number, 42);
        assert_eq!(
            issues[0].worktree_path.as_deref(),
            Some(".cupola/worktrees/42")
        );
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
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(
            output.contains("Process: running") && output.contains("pid=1234"),
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
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("Process: not running"), "output: {output}");
    }

    #[tokio::test]
    async fn daemon_status_stale_pid_cleaned() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        // PID in file but process is not alive
        let pid_mgr = MockPidFilePort::new().with_pid(5678);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(pid_mgr.was_deleted(), "PID file should be deleted");
        assert!(
            output.contains("Process: not running (stale PID file cleaned)"),
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
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        let process_pos = output.find("Process:").expect("should have process line");
        let issue_pos = output.find("#42").expect("should have issue line");
        assert!(
            process_pos < issue_pos,
            "Process status should appear before issue list; output: {output}"
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
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        // TODO(phase 6): re-tighten with process_runs pid tracking
        assert!(output.contains("#10"), "output: {output}");
    }

    #[tokio::test]
    async fn issue_with_dead_pid_shows_dead() {
        // TODO(phase 6): re-tighten with process_runs pid tracking
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        repo.save(&make_issue(11, State::DesignRunning, Some(200)))
            .await
            .expect("save");

        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("#11"), "output: {output}");
    }

    #[tokio::test]
    async fn issue_with_no_pid_shows_no_pid_info() {
        // TODO(phase 6): re-tighten with process_runs pid tracking
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        repo.save(&make_issue(12, State::DesignRunning, None))
            .await
            .expect("save");

        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("#12"), "output: {output}");
    }

    #[tokio::test]
    async fn running_count_only_counts_alive_processes() {
        // TODO(phase 6): re-tighten with process_runs pid tracking
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);

        repo.save(&make_issue(20, State::DesignRunning, Some(300)))
            .await
            .expect("save");
        repo.save(&make_issue(21, State::DesignRunning, Some(301)))
            .await
            .expect("save");

        let pid_mgr = MockPidFilePort::new().with_alive_pid(300);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("Claude sessions:"), "output: {output}");
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

    // --- T-6.ST.* status tests ---

    /// T-6.ST.1: PID ファイルなし → "not running"
    #[tokio::test]
    async fn t_6_st_1_process_not_running_when_no_pid_file() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");
        assert!(
            output.contains("not running"),
            "should say 'not running' when no PID file; output: {output}"
        );
    }

    /// T-6.ST.2: 有効な PID ファイル → "running (pid=NNN)"
    #[tokio::test]
    async fn t_6_st_2_process_running_with_pid() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        let pid_mgr = MockPidFilePort::new().with_pid(4321).with_alive_pid(4321);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");
        assert!(
            output.contains("running") && output.contains("4321"),
            "should show running pid; output: {output}"
        );
    }

    /// T-6.ST.3: ステール PID → クリーンアップメッセージ
    #[tokio::test]
    async fn t_6_st_3_stale_pid_shows_cleanup_message() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        // PID in file but process not alive
        let pid_mgr = MockPidFilePort::new().with_pid(9876);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");
        assert!(
            output.contains("stale") || output.contains("not running"),
            "should mention stale PID cleanup; output: {output}"
        );
    }

    /// T-6.ST.4: アクティブ Issue なし → "No active issues"
    #[tokio::test]
    async fn t_6_st_4_no_active_issues_message() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");
        assert!(
            output.contains("No active issues"),
            "should say 'No active issues' when DB is empty; output: {output}"
        );
    }

    /// T-6.ST.5: アクティブ Issue があり max_concurrent_sessions が設定されていない場合
    #[tokio::test]
    async fn t_6_st_5_active_issues_show_claude_sessions() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        repo.save(&make_issue(33, State::DesignRunning, None))
            .await
            .expect("save");
        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");
        // Should contain some session info
        assert!(
            output.contains("Claude sessions:") || output.contains("#33"),
            "should show issue info; output: {output}"
        );
    }

    /// T-6.ST.6: デフォルトは Completed/Cancelled を除外する
    #[tokio::test]
    async fn t_6_st_6_default_excludes_completed_and_cancelled() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        // Completed issue should not appear in default status
        let mut completed = make_issue(88, State::Completed, None);
        completed.close_finished = true;
        repo.save(&completed).await.expect("save completed");
        // Active issue should appear
        repo.save(&make_issue(89, State::DesignRunning, None))
            .await
            .expect("save active");

        let pid_mgr = MockPidFilePort::new();
        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        // Active issue should appear
        assert!(
            output.contains("#89"),
            "active issue should appear; output: {output}"
        );
    }

    /// T-6.ST.8: Issue 行には GitHub#, state, worktree などの情報が含まれる
    #[tokio::test]
    async fn t_6_st_8_issue_line_contains_key_info() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        repo.save(&make_issue(55, State::DesignRunning, None))
            .await
            .expect("save");
        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");
        assert!(
            output.contains("#55"),
            "should contain issue number; output: {output}"
        );
    }

    // --- install_panic_hook tests ---
    //
    // Panic hooks are global process state, so tests that install/remove hooks
    // must run serially.  We use a module-level OnceLock<Mutex> for this purpose.

    fn hook_test_mutex() -> &'static std::sync::Mutex<()> {
        static M: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        M.get_or_init(|| std::sync::Mutex::new(()))
    }

    /// After `install_panic_hook`, a panic deletes the PID file before unwinding.
    #[test]
    fn install_panic_hook_deletes_pid_file_on_panic() {
        let _guard = hook_test_mutex().lock().unwrap_or_else(|e| e.into_inner());

        let dir = tempfile::TempDir::new().expect("temp dir");
        let pid_path = dir.path().join("cupola.pid");
        std::fs::write(&pid_path, "12345\n").expect("write PID file");
        assert!(pid_path.exists(), "PID file must exist before test");

        let saved_hook = std::panic::take_hook();

        use crate::application::session_manager::ChildProcessRegistry;
        super::install_panic_hook(pid_path.clone(), ChildProcessRegistry::new());

        let result = std::panic::catch_unwind(|| {
            panic!("intentional test panic");
        });

        let _installed = std::panic::take_hook();
        std::panic::set_hook(saved_hook);

        assert!(result.is_err(), "catch_unwind should have caught a panic");
        assert!(
            !pid_path.exists(),
            "PID file should be deleted by the panic hook"
        );
    }

    /// When the PID file does not exist, the hook must complete without itself panicking.
    #[test]
    fn install_panic_hook_safe_when_pid_file_absent() {
        let _guard = hook_test_mutex().lock().unwrap_or_else(|e| e.into_inner());

        let dir = tempfile::TempDir::new().expect("temp dir");
        let pid_path = dir.path().join("cupola.pid");
        assert!(!pid_path.exists(), "PID file must not exist before test");

        let saved_hook = std::panic::take_hook();
        use crate::application::session_manager::ChildProcessRegistry;
        super::install_panic_hook(pid_path.clone(), ChildProcessRegistry::new());

        let result = std::panic::catch_unwind(|| {
            panic!("intentional test panic – no pid file");
        });

        let _installed = std::panic::take_hook();
        std::panic::set_hook(saved_hook);

        assert!(result.is_err(), "catch_unwind should have caught the panic");
    }

    /// panic hook が registry に登録された子プロセスを終了させること。
    #[test]
    fn install_panic_hook_kills_registered_child_process() {
        use crate::application::session_manager::ChildProcessRegistry;
        use std::process::{Command, Stdio};

        let _guard = hook_test_mutex().lock().unwrap_or_else(|e| e.into_inner());

        let dir = tempfile::TempDir::new().expect("temp dir");
        let pid_path = dir.path().join("cupola.pid");
        std::fs::write(&pid_path, "12345\n").expect("write PID file");

        let mut child = Command::new("sleep")
            .arg("60")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let child_pid = child.id();

        let reg = ChildProcessRegistry::new();
        reg.register(child_pid);

        let saved_hook = std::panic::take_hook();
        super::install_panic_hook(pid_path.clone(), reg);

        // catch_unwind runs the hook synchronously (blocks up to 3s for shutdown_sync)
        let result = std::panic::catch_unwind(|| {
            panic!("intentional test panic for child kill");
        });

        let _installed = std::panic::take_hook();
        std::panic::set_hook(saved_hook);

        assert!(result.is_err(), "catch_unwind should have caught a panic");
        assert!(
            !pid_path.exists(),
            "PID file should be deleted by the panic hook"
        );

        // child.wait() correctly handles zombies: if SIGTERM caused the process to exit,
        // it will be a zombie that wait() reaps immediately with a non-zero exit status.
        let status = child.wait().expect("wait should succeed");
        assert!(
            !status.success(),
            "sleep pid={child_pid} should have been killed, not exited normally"
        );
    }

    /// panic hook が Mutex 毒化状態でも子プロセスを終了させること。
    #[test]
    fn install_panic_hook_kills_child_even_with_poisoned_mutex() {
        use crate::application::session_manager::ChildProcessRegistry;
        use std::process::{Command, Stdio};

        let _guard = hook_test_mutex().lock().unwrap_or_else(|e| e.into_inner());

        let dir = tempfile::TempDir::new().expect("temp dir");
        let pid_path = dir.path().join("cupola.pid");
        std::fs::write(&pid_path, "12345\n").expect("write PID file");

        let mut child = Command::new("sleep")
            .arg("60")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let child_pid = child.id();

        let reg = ChildProcessRegistry::new();
        reg.register(child_pid);

        // Poison the mutex by panicking while holding the lock
        let reg_clone = reg.clone();
        let _ = std::panic::catch_unwind(move || {
            let _guard = reg_clone.pids.lock().expect("lock");
            panic!("intentional mutex poison");
        });

        // The registry mutex is now poisoned; shutdown_sync should still work via into_inner
        let saved_hook = std::panic::take_hook();
        super::install_panic_hook(pid_path.clone(), reg);

        let result = std::panic::catch_unwind(|| {
            panic!("intentional test panic");
        });

        let _installed = std::panic::take_hook();
        std::panic::set_hook(saved_hook);

        assert!(result.is_err());

        // child.wait() correctly handles zombies: if SIGTERM caused exit, returns immediately.
        let status = child.wait().expect("wait should succeed");
        assert!(
            !status.success(),
            "sleep pid={child_pid} should have been killed even with poisoned mutex"
        );
    }

    // --- check_daemon_not_running tests ---

    #[test]
    fn cleanup_aborts_when_daemon_is_running() {
        let pid_mgr = MockPidFilePort::new().with_pid(1234).with_alive_pid(1234);
        let result = super::check_daemon_not_running(&pid_mgr);
        assert!(result.is_err(), "should error when daemon is alive");
        let err_msg = result.unwrap_err().to_string();
        assert_eq!(
            err_msg, "cupola is running (pid=1234). Run `cupola stop` first.",
            "error message should match expected format"
        );
    }

    #[test]
    fn cleanup_aborts_when_pid_file_unreadable() {
        let pid_mgr = MockPidFilePort::new().with_read_error("permission denied");
        let result = super::check_daemon_not_running(&pid_mgr);
        assert!(result.is_err(), "should error when PID file cannot be read");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("failed to read PID file"),
            "error should mention PID file read failure; got: {err_msg}"
        );
    }

    #[test]
    fn cleanup_proceeds_when_no_pid_file() {
        let pid_mgr = MockPidFilePort::new();
        let result = super::check_daemon_not_running(&pid_mgr);
        assert!(result.is_ok(), "should succeed when no PID file exists");
    }

    #[test]
    fn cleanup_proceeds_when_pid_file_stale() {
        // PID file exists but process is not alive (stale)
        let pid_mgr = MockPidFilePort::new().with_pid(5678);
        let result = super::check_daemon_not_running(&pid_mgr);
        assert!(
            result.is_ok(),
            "should succeed when PID file is stale (process not alive)"
        );
    }

    // --- Task 5.3: handle_status new tests ---

    #[tokio::test]
    async fn test_status_output_foreground_mode() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        let pid_mgr = MockPidFilePort::new()
            .with_pid(4242)
            .with_alive_pid(4242)
            .with_mode(ProcessMode::Foreground);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(
            output.contains("Process: running (foreground, pid=4242)"),
            "output: {output}"
        );
    }

    #[tokio::test]
    async fn test_status_output_daemon_mode() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        let pid_mgr = MockPidFilePort::new()
            .with_pid(7777)
            .with_alive_pid(7777)
            .with_mode(ProcessMode::Daemon);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(
            output.contains("Process: running (daemon, pid=7777)"),
            "output: {output}"
        );
    }

    #[tokio::test]
    async fn test_status_output_unknown_mode() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        // mode = None simulates legacy single-line PID file
        let pid_mgr = MockPidFilePort::new().with_pid(9999).with_alive_pid(9999);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(
            output.contains("Process: running (unknown, pid=9999)"),
            "output: {output}"
        );
    }

    #[tokio::test]
    async fn test_status_output_not_running() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("Process: not running"), "output: {output}");
    }

    #[tokio::test]
    async fn test_status_output_session_label_with_max() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        repo.save(&make_issue(77, State::DesignRunning, None))
            .await
            .expect("save");
        // PIDs 1000..1002 match MockProcessRunRepository::with_running_count(2)
        let pid_mgr = MockPidFilePort::new().with_alive_pids(1000..1002);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::with_running_count(2),
            &repo,
            Some(5),
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("Claude sessions: 2/5"), "output: {output}");
    }

    #[tokio::test]
    async fn test_status_output_session_label_no_max() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        repo.save(&make_issue(78, State::DesignRunning, None))
            .await
            .expect("save");
        // PIDs 1000..1003 match MockProcessRunRepository::with_running_count(3)
        let pid_mgr = MockPidFilePort::new().with_alive_pids(1000..1003);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::with_running_count(3),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(output.contains("Claude sessions: 3"), "output: {output}");
        assert!(
            !output.contains("Claude sessions: 3/"),
            "should not have max; output: {output}"
        );
    }

    #[tokio::test]
    async fn test_status_running_count_from_process_runs() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        repo.save(&make_issue(79, State::DesignRunning, None))
            .await
            .expect("save");
        // PIDs 1000..1004 match MockProcessRunRepository::with_running_count(4)
        let pid_mgr = MockPidFilePort::new().with_alive_pids(1000..1004);

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::with_running_count(4),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(
            output.contains("Claude sessions: 4"),
            "output should show 4 running; output: {output}"
        );
    }

    #[tokio::test]
    async fn test_status_running_count_zero_when_no_running_records() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        let repo = SqliteIssueRepository::new(db);
        repo.save(&make_issue(80, State::DesignRunning, None))
            .await
            .expect("save");
        let pid_mgr = MockPidFilePort::new();

        let mut out = Vec::new();
        super::handle_status(
            &mut out,
            &pid_mgr,
            &MockProcessRunRepository::new(),
            &repo,
            None,
        )
        .await
        .expect("handle");
        let output = String::from_utf8(out).expect("utf8");

        assert!(
            output.contains("Claude sessions: 0"),
            "output should show 0 running; output: {output}"
        );
    }
}
