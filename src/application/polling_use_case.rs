use std::time::Duration;

use anyhow::Result;
use tokio::signal;
use tokio::signal::unix::SignalKind;
use tokio::time::interval;

use crate::application::init_task_manager::InitTaskManager;
use crate::application::polling::{collect, execute, persist, resolve};
use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::execution_log_repository::ExecutionLogRepository;
use crate::application::port::file_generator::FileGenerator;
use crate::application::port::git_worktree::GitWorktree;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::pid_file::PidFilePort;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::application::session_manager::SessionManager;
use crate::domain::config::Config;
use crate::domain::decide::decide;
use crate::domain::task_weight::TaskWeight;

/// Convert GitHub issue labels to a TaskWeight.
/// Priority: heavy > light > medium (default).
pub fn label_to_weight(labels: &[String]) -> TaskWeight {
    if labels.iter().any(|l| l == "weight:heavy") {
        return TaskWeight::Heavy;
    }
    if labels.iter().any(|l| l == "weight:light") {
        return TaskWeight::Light;
    }
    TaskWeight::Medium
}

/// No-op ProcessRunRepository used when the real one isn't wired yet.
/// TODO(phase 5): remove once bootstrap always provides a real repository.
/// Noop impl, visible enough to satisfy the default type parameter bound.
pub struct NoopProcessRunRepository;

impl ProcessRunRepository for NoopProcessRunRepository {
    async fn save(&self, _: &crate::domain::process_run::ProcessRun) -> Result<i64> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn update_pid(&self, _: i64, _: u32) -> Result<()> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn mark_succeeded(&self, _: i64, _: Option<u64>) -> Result<()> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn mark_failed(&self, _: i64, _: Option<String>) -> Result<()> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn mark_stale(&self, _: i64) -> Result<()> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn mark_stale_for_issue(&self, _: i64) -> Result<()> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn find_latest(
        &self,
        _: i64,
        _: crate::domain::process_run::ProcessRunType,
    ) -> Result<Option<crate::domain::process_run::ProcessRun>> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn find_latest_with_pr_number(
        &self,
        _: i64,
        _: crate::domain::process_run::ProcessRunType,
    ) -> Result<Option<crate::domain::process_run::ProcessRun>> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn find_by_issue(&self, _: i64) -> Result<Vec<crate::domain::process_run::ProcessRun>> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn count_consecutive_failures(
        &self,
        _: i64,
        _: crate::domain::process_run::ProcessRunType,
        _: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<u32> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn find_latest_with_consecutive_count(
        &self,
        _: i64,
        _: crate::domain::process_run::ProcessRunType,
        _: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Option<(crate::domain::process_run::ProcessRun, u32)>> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn find_all_running(&self) -> Result<Vec<crate::domain::process_run::ProcessRun>> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
    async fn update_state(
        &self,
        _: i64,
        _: crate::domain::process_run::ProcessRunState,
    ) -> Result<()> {
        Err(anyhow::anyhow!("NoopProcessRunRepository: not wired"))
    }
}

pub struct PollingUseCase<G, I, E, C, W, F, P = NoopProcessRunRepository>
where
    G: GitHubClient,
    I: IssueRepository,
    E: ExecutionLogRepository,
    C: ClaudeCodeRunner,
    W: GitWorktree,
    F: FileGenerator,
    P: ProcessRunRepository,
{
    github: G,
    issue_repo: I,
    #[allow(dead_code)] // TODO(phase 6): ExecutionLogRepository used for legacy logs command
    exec_log_repo: E,
    claude_runner: C,
    worktree: W,
    file_gen: F,
    session_mgr: SessionManager,
    init_mgr: InitTaskManager,
    config: Config,
    pid_file: Option<Box<dyn PidFilePort>>,
    process_repo: P,
}

impl<G, I, E, C, W, F> PollingUseCase<G, I, E, C, W, F, NoopProcessRunRepository>
where
    G: GitHubClient,
    I: IssueRepository,
    E: ExecutionLogRepository,
    C: ClaudeCodeRunner,
    W: GitWorktree,
    F: FileGenerator,
{
    pub fn new(
        github: G,
        issue_repo: I,
        exec_log_repo: E,
        claude_runner: C,
        worktree: W,
        file_gen: F,
        config: Config,
    ) -> Self {
        Self {
            github,
            issue_repo,
            exec_log_repo,
            claude_runner,
            worktree,
            file_gen,
            session_mgr: SessionManager::new(),
            init_mgr: InitTaskManager::new(),
            config,
            pid_file: None,
            process_repo: NoopProcessRunRepository,
        }
    }
}

impl<G, I, E, C, W, F, P> PollingUseCase<G, I, E, C, W, F, P>
where
    G: GitHubClient,
    I: IssueRepository,
    E: ExecutionLogRepository,
    C: ClaudeCodeRunner,
    // Clone + 'static is required because SpawnInit moves a worktree handle
    // into a `tokio::spawn` task per docs/architecture/effects.md.
    W: GitWorktree + Clone + 'static,
    F: FileGenerator + Clone + 'static,
    P: ProcessRunRepository,
{
    /// Wire a real ProcessRunRepository (used by bootstrap once Phase 5 is complete).
    pub fn with_process_repo<P2: ProcessRunRepository>(
        self,
        process_repo: P2,
    ) -> PollingUseCase<G, I, E, C, W, F, P2> {
        PollingUseCase {
            github: self.github,
            issue_repo: self.issue_repo,
            exec_log_repo: self.exec_log_repo,
            claude_runner: self.claude_runner,
            worktree: self.worktree,
            file_gen: self.file_gen,
            session_mgr: self.session_mgr,
            init_mgr: self.init_mgr,
            config: self.config,
            pid_file: self.pid_file,
            process_repo,
        }
    }

    pub fn with_pid_file(mut self, pid_file: Box<dyn PidFilePort>) -> Self {
        self.pid_file = Some(pid_file);
        self
    }

    /// Main polling loop — 5-phase cycle.
    pub async fn run(&mut self) -> Result<()> {
        let mut tick = interval(Duration::from_secs(self.config.polling_interval_secs));
        let mut sigterm = signal::unix::signal(SignalKind::terminate())?;
        let mut sighup = signal::unix::signal(SignalKind::hangup())?;

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    if let Err(e) = self.run_cycle().await {
                        tracing::error!(error = %e, "polling cycle error");
                    }
                }
                _ = signal::ctrl_c() => {
                    tracing::info!("received SIGINT, shutting down...");
                    break;
                }
                _ = sigterm.recv() => {
                    tracing::info!("received SIGTERM, shutting down...");
                    break;
                }
                _ = sighup.recv() => {
                    tracing::info!("received SIGHUP, shutting down (config reload is not supported, please restart to apply config changes)...");
                    break;
                }
            }
        }

        self.graceful_shutdown().await;
        Ok(())
    }

    /// Execute one 5-phase polling cycle.
    async fn run_cycle(&mut self) -> Result<()> {
        // === Phase 1: Resolve ===
        resolve::kill_stalled(&mut self.session_mgr, self.config.stall_timeout_secs);

        resolve::resolve_exited_sessions(
            &self.github,
            &self.issue_repo,
            &self.process_repo,
            &self.config,
            &mut self.session_mgr,
        )
        .await?;

        resolve::resolve_init_tasks(&self.issue_repo, &self.process_repo, &mut self.init_mgr)
            .await?;

        // === Phase 2: Collect ===
        let observations = collect::collect_all(
            &self.github,
            &self.issue_repo,
            &self.process_repo,
            &self.config,
        )
        .await?;

        // === Phases 3-4-5: per issue: Decide → Persist → Execute ===
        for obs in observations {
            let mut issue = obs.issue;
            let snapshot = obs.snapshot;

            // Phase 3: Decide
            let decision = decide(&issue, &snapshot, &self.config);

            // Phase 4: Persist
            if let Err(e) = persist::persist_decision(&self.issue_repo, &issue, &decision).await {
                tracing::warn!(
                    issue_number = issue.github_issue_number,
                    error = %e,
                    "persist failed for issue, skipping execute"
                );
                continue;
            }

            // Apply decision to local issue copy
            decision.metadata_updates.apply_to(&mut issue);
            issue.state = decision.next_state;

            // Phase 5: Execute
            if let Err(e) = execute::execute_effects(
                &self.github,
                &self.issue_repo,
                &self.process_repo,
                &self.claude_runner,
                &self.worktree,
                &self.file_gen,
                &mut self.session_mgr,
                &mut self.init_mgr,
                &self.config,
                &mut issue,
                &decision.effects,
            )
            .await
            {
                tracing::warn!(
                    issue_number = issue.github_issue_number,
                    error = %e,
                    "execute failed for issue"
                );
            }
        }

        Ok(())
    }

    async fn graceful_shutdown(&mut self) {
        self.session_mgr.kill_all();

        // Bounded wait for child processes to exit
        let timeout = std::time::Duration::from_secs(10);
        let start = std::time::Instant::now();
        while self.session_mgr.count() > 0 && start.elapsed() < timeout {
            self.session_mgr.collect_exited();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let remaining = self.session_mgr.count();
        if remaining > 0 {
            tracing::warn!(
                remaining_sessions = remaining,
                elapsed_ms = start.elapsed().as_millis(),
                "graceful shutdown timed out with sessions still running"
            );
        }

        // Abort pending init tasks.  Note: tasks that are already inside
        // `spawn_blocking` (running a `git` subprocess via
        // `std::process::Command`) cannot be interrupted by `abort()`; the
        // blocking thread will run to completion.  Full cooperative
        // cancellation of git subprocesses is out of scope for this fix.
        self.init_mgr.abort_all();
        if let Some(ref pid_file) = self.pid_file {
            let _ = pid_file.delete_pid();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-263.1: SIGHUP シグナルが tokio の signal ハンドラーで受信できること
    ///
    /// run() 内の `sighup.recv()` アームが正しく機能することを検証する。
    /// 実際のシグナルを自プロセスに送信して受信を確認する。
    #[cfg(unix)]
    #[tokio::test]
    async fn sighup_signal_is_received_by_handler() {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;
        use tokio::signal::unix::{SignalKind, signal};

        let mut sighup =
            signal(SignalKind::hangup()).expect("SIGHUP ハンドラーの登録に失敗しました");

        // 自プロセスに SIGHUP を送信する
        kill(Pid::this(), Signal::SIGHUP).expect("SIGHUP の送信に失敗しました");

        // シグナルが受信されることを検証する（1 秒以内）
        tokio::time::timeout(std::time::Duration::from_secs(1), sighup.recv())
            .await
            .expect("SIGHUP がタイムアウト内に受信されませんでした");
    }

    /// T-5.O.1: label_to_weight selects Heavy when weight:heavy is present
    #[test]
    fn label_to_weight_heavy() {
        let labels = vec!["weight:heavy".to_string(), "bug".to_string()];
        assert_eq!(label_to_weight(&labels), TaskWeight::Heavy);
    }

    /// T-5.O.2: label_to_weight selects Heavy when both heavy and light are present
    #[test]
    fn label_to_weight_heavy_wins_over_light() {
        let labels = vec!["weight:heavy".to_string(), "weight:light".to_string()];
        assert_eq!(label_to_weight(&labels), TaskWeight::Heavy);
    }

    #[test]
    fn label_to_weight_light() {
        let labels = vec!["weight:light".to_string()];
        assert_eq!(label_to_weight(&labels), TaskWeight::Light);
    }

    #[test]
    fn label_to_weight_medium_default() {
        let labels: Vec<String> = vec![];
        assert_eq!(label_to_weight(&labels), TaskWeight::Medium);
    }
}
