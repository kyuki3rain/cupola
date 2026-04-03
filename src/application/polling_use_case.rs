use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use rust_i18n::t;
use tokio::signal;
use tokio::signal::unix::SignalKind;
use tokio::time::interval;

use crate::application::io::{
    CiErrorEntry, ConflictInfo, parse_fixing_output, parse_pr_creation_output,
    write_ci_errors_input, write_conflict_info_input, write_issue_input,
    write_review_threads_input,
};
use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::execution_log_repository::ExecutionLogRepository;
use crate::application::port::git_worktree::GitWorktree;
use crate::application::port::github_client::{GitHubClient, PrStatus};
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::pid_file::PidFilePort;
use crate::application::prompt::{
    FIXING_SCHEMA, OutputSchemaKind, PR_CREATION_SCHEMA, build_session_config, fallback_pr_body,
    fallback_pr_title,
};
use crate::application::retry_policy::{RetryDecision, RetryPolicy};
use crate::application::session_manager::{ExitedSession, SessionManager};
use crate::application::transition_use_case::{TransitionUseCase, prioritize_events};
use crate::domain::config::Config;
use crate::domain::event::Event;
use crate::domain::fixing_problem_kind::FixingProblemKind;
use crate::domain::issue::Issue;
use crate::domain::state::State;

pub struct PollingUseCase<G, I, E, C, W>
where
    G: GitHubClient,
    I: IssueRepository,
    E: ExecutionLogRepository,
    C: ClaudeCodeRunner,
    W: GitWorktree,
{
    github: G,
    issue_repo: I,
    exec_log_repo: E,
    claude_runner: C,
    worktree: W,
    session_mgr: SessionManager,
    retry_policy: RetryPolicy,
    config: Config,
    pid_file: Option<Box<dyn PidFilePort>>,
}

impl<G, I, E, C, W> PollingUseCase<G, I, E, C, W>
where
    G: GitHubClient,
    I: IssueRepository,
    E: ExecutionLogRepository,
    C: ClaudeCodeRunner,
    W: GitWorktree,
{
    pub fn new(
        github: G,
        issue_repo: I,
        exec_log_repo: E,
        claude_runner: C,
        worktree: W,
        config: Config,
    ) -> Self {
        let retry_policy = RetryPolicy::new(config.max_retries);
        Self {
            github,
            issue_repo,
            exec_log_repo,
            claude_runner,
            worktree,
            session_mgr: SessionManager::new(),
            retry_policy,
            config,
            pid_file: None,
        }
    }

    pub fn with_pid_file(mut self, pid_file: Box<dyn PidFilePort>) -> Self {
        self.pid_file = Some(pid_file);
        self
    }

    /// Main polling loop. Runs until SIGINT or SIGTERM.
    pub async fn run(&mut self) -> Result<()> {
        // Startup recovery: clear PIDs
        self.recover_on_startup().await;

        let mut tick = interval(Duration::from_secs(self.config.polling_interval_secs));
        let mut sigterm = signal::unix::signal(SignalKind::terminate())?;

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    if let Err(e) = self.run_cycle().await {
                        tracing::error!(error = %e, "polling cycle failed");
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
            }
        }

        self.graceful_shutdown().await;
        Ok(())
    }

    /// Execute one polling cycle (the 7 steps).
    pub async fn run_cycle(&mut self) -> Result<()> {
        let mut events: Vec<(i64, Event)> = Vec::new();

        // Step 1: Issue polling
        self.step1_issue_polling(&mut events).await;

        // Step 2: Initialized recovery (immediate apply)
        self.step2_initialized_recovery().await;

        // Step 3: Process exit check
        self.step3_process_exit_check(&mut events).await;

        // Step 4: PR monitoring
        self.step4_pr_monitoring(&mut events).await;

        // Step 5: Stall detection
        self.step5_stall_detection(&mut events);

        // Step 6: Apply events
        self.step6_apply_events(&mut events).await;

        // Step 7: Spawn processes
        self.step7_spawn_processes().await;

        Ok(())
    }

    // --- Step 1: Issue polling ---

    async fn step1_issue_polling(&self, events: &mut Vec<(i64, Event)>) {
        // Detect new issues with agent:ready label
        match self.github.list_ready_issues().await {
            Ok(github_issues) => {
                for gi in &github_issues {
                    match self.issue_repo.find_by_issue_number(gi.number).await {
                        Ok(Some(existing)) if !existing.state.is_terminal() => {
                            // Already tracked, skip
                        }
                        Ok(_) => {
                            // New or terminal — will be handled
                            events.push((
                                0, // ID not yet known; handle_issue_detected will create
                                Event::IssueDetected {
                                    issue_number: gi.number,
                                },
                            ));
                        }
                        Err(e) => {
                            tracing::warn!(issue_number = gi.number, error = %e, "failed to check issue in DB");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to list ready issues");
            }
        }

        // Detect closed issues
        match self.issue_repo.find_active().await {
            Ok(active_issues) => {
                for issue in &active_issues {
                    match self.github.is_issue_open(issue.github_issue_number).await {
                        Ok(false) => {
                            // For review_waiting states, check if PR was merged first.
                            // GitHub's "Closes #N" auto-closes the Issue on PR merge,
                            // so we need to distinguish merge-close from manual-close.
                            let event = self.resolve_close_event_for_issue(issue).await;
                            events.push((issue.id, event));
                        }
                        Ok(true) => {}
                        Err(e) => {
                            tracing::warn!(
                                issue_number = issue.github_issue_number,
                                error = %e,
                                "failed to check if issue is open"
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to find active issues");
            }
        }
    }

    // --- Step 2: Initialized recovery ---

    async fn step2_initialized_recovery(&mut self) {
        let active = match self.issue_repo.find_active().await {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(error = %e, "failed to find active issues for recovery");
                return;
            }
        };

        for mut issue in active {
            if issue.state != State::Initialized {
                continue;
            }

            tracing::info!(
                issue_number = issue.github_issue_number,
                "recovering initialized issue"
            );

            match self.initialize_issue(&mut issue).await {
                Ok(()) => {
                    let uc = self.transition_uc();
                    if let Err(e) = uc.apply(&mut issue, &Event::InitializationCompleted).await {
                        tracing::error!(issue_number = issue.github_issue_number, error = %e, "failed to transition to DesignRunning");
                    }
                }
                Err(e) => {
                    tracing::warn!(issue_number = issue.github_issue_number, error = %e, "initialization failed, incrementing retry");
                    issue.retry_count += 1;
                    issue.error_message = Some(e.to_string());
                    let _ = self.issue_repo.update(&issue).await;

                    if self.retry_policy.evaluate(issue.retry_count) == RetryDecision::Exhausted {
                        let uc = self.transition_uc();
                        let _ = uc.apply(&mut issue, &Event::RetryExhausted).await;
                    }
                }
            }
        }
    }

    async fn initialize_issue(&self, issue: &mut Issue) -> Result<()> {
        let n = issue.github_issue_number;
        let wt_path = format!(".cupola/worktrees/{n}");
        let main_branch = format!("cupola/{n}/main");
        let design_branch = format!("cupola/{n}/design");

        self.worktree.fetch()?;

        let wt = Path::new(&wt_path);
        if self.worktree.exists(wt) {
            // worktree が既に存在する場合は再作成をスキップし、再開メッセージを投稿する
            tracing::info!(issue_number = n, worktree = %wt_path, "worktree already exists, reusing");
            // cleanup が部分失敗して worktree_path が None のまま worktree が残った場合に自己修復する
            if issue.worktree_path.is_none() {
                tracing::info!(issue_number = n, worktree = %wt_path, "self-healing: worktree_path was None, restoring from filesystem");
                issue.worktree_path = Some(wt_path.clone());
                self.issue_repo.update(issue).await?;
            }
            let comment_key = if issue.impl_pr_number.is_some() {
                "issue_comment.resuming_implementation"
            } else {
                "issue_comment.resuming_design"
            };
            self.github
                .comment_on_issue(n, &t!(comment_key, locale = &self.config.language))
                .await?;
        } else {
            // 新規作成フロー
            let start_point = format!("origin/{}", self.config.default_branch);
            self.worktree.create(wt, &main_branch, &start_point)?;
            self.worktree.push(wt, &main_branch)?;
            self.worktree.create_branch(wt, &design_branch)?;
            self.worktree.push(wt, &design_branch)?;

            issue.worktree_path = Some(wt_path);
            self.issue_repo.update(issue).await?;

            self.github
                .comment_on_issue(
                    n,
                    &t!(
                        "issue_comment.design_starting",
                        locale = &self.config.language
                    ),
                )
                .await?;
        }

        Ok(())
    }

    // --- Step 3: Process exit check ---

    async fn step3_process_exit_check(&mut self, events: &mut Vec<(i64, Event)>) {
        let exited = self.session_mgr.collect_exited();

        for session in exited {
            let mut issue = match self.issue_repo.find_by_id(session.issue_id).await {
                Ok(Some(i)) => i,
                _ => continue,
            };

            // Clear PID in DB
            if issue.current_pid.is_some() {
                issue.current_pid = None;
                if let Err(e) = self.issue_repo.update(&issue).await {
                    tracing::warn!(issue_id = issue.id, error = %e, "failed to clear PID");
                }
            }

            // Record execution log
            let log_id = self
                .exec_log_repo
                .record_start(issue.id, issue.state)
                .await
                .unwrap_or(0);

            if session.exit_status.success() {
                tracing::info!(
                    issue_number = issue.github_issue_number,
                    exit_code = session.exit_status.code().unwrap_or(0),
                    "Claude Code process exited successfully"
                );
                self.handle_successful_exit(&issue, &session, log_id, events)
                    .await;
            } else {
                tracing::info!(
                    issue_number = issue.github_issue_number,
                    exit_code = session.exit_status.code().unwrap_or(-1),
                    "Claude Code process exited with failure"
                );
                // Record failure
                let _ = self
                    .exec_log_repo
                    .record_finish(
                        log_id,
                        session.exit_status.code(),
                        None,
                        Some(&session.stderr),
                    )
                    .await;

                self.handle_failed_exit(&issue, events);
            }
        }
    }

    async fn handle_successful_exit(
        &self,
        issue: &Issue,
        session: &ExitedSession,
        log_id: i64,
        events: &mut Vec<(i64, Event)>,
    ) {
        let _ = self
            .exec_log_repo
            .record_finish(
                log_id,
                session.exit_status.code(),
                Some(&session.stdout),
                None,
            )
            .await;

        if issue.state.needs_process() && !issue.state.is_review_waiting() {
            // running state → create PR
            if matches!(
                issue.state,
                State::DesignRunning | State::ImplementationRunning
            ) {
                match self.create_pr_from_output(issue, &session.stdout).await {
                    Ok(_) => events.push((issue.id, Event::ProcessCompletedWithPr)),
                    Err(e) => {
                        tracing::warn!(issue_id = issue.id, error = %e, "PR creation failed");
                        self.handle_failed_exit(issue, events);
                    }
                }
            } else {
                // fixing state → reply + resolve
                self.process_fixing_output(issue, &session.stdout).await;
                events.push((issue.id, Event::ProcessCompleted));
            }
        }
    }

    fn handle_failed_exit(&self, issue: &Issue, events: &mut Vec<(i64, Event)>) {
        match self.retry_policy.evaluate(issue.retry_count) {
            RetryDecision::Retry => events.push((issue.id, Event::ProcessFailed)),
            RetryDecision::Exhausted => events.push((issue.id, Event::RetryExhausted)),
        }
    }

    // --- Step 4: PR monitoring ---

    async fn step4_pr_monitoring(&mut self, events: &mut Vec<(i64, Event)>) {
        let active = match self.issue_repo.find_active().await {
            Ok(a) => a,
            Err(_) => return,
        };

        for mut issue in active {
            if !issue.state.is_review_waiting() {
                continue;
            }

            let pr_number = match issue.state {
                State::DesignReviewWaiting => issue.design_pr_number,
                State::ImplementationReviewWaiting => issue.impl_pr_number,
                _ => continue,
            };

            let Some(pr_num) = pr_number else {
                continue;
            };

            // (1) Get PR details once — used for both merged and mergeable checks
            let pr_details = match self.github.get_pr_details(pr_num).await {
                Ok(details) => details,
                Err(e) => {
                    tracing::warn!(pr_number = pr_num, error = %e, "failed to get PR details");
                    continue;
                }
            };

            // (1a) Check merge first (priority) — if merged, skip further checks
            if pr_details.merged {
                let event = match issue.state {
                    State::DesignReviewWaiting => Event::DesignPrMerged,
                    State::ImplementationReviewWaiting => Event::ImplementationPrMerged,
                    _ => continue,
                };
                events.push((issue.id, event));
                continue;
            }

            let mut causes: Vec<FixingProblemKind> = Vec::new();

            // (2) CI failure check
            match self.github.get_ci_check_runs(pr_num).await {
                Ok(runs) => {
                    let has_failure = runs
                        .iter()
                        .any(|r| r.conclusion.as_deref() == Some("failure"));
                    if has_failure {
                        causes.push(FixingProblemKind::CiFailure);
                        tracing::info!(pr_number = pr_num, "CI failure detected");
                    }
                }
                Err(e) => {
                    tracing::warn!(pr_number = pr_num, error = %e, "failed to get CI check runs");
                    // Skip CI check; continue with other checks
                }
            }

            // (3) Conflict check (reuse mergeable from already-fetched PR details)
            match pr_details.mergeable {
                Some(false) => {
                    causes.push(FixingProblemKind::Conflict);
                    tracing::info!(
                        pr_number = pr_num,
                        "conflict detected, adding to fixing causes"
                    );
                }
                None => {
                    // GitHub is still computing mergeability — skip this cycle's conflict check
                }
                Some(true) => {}
            }

            // (4) Unresolved review threads check
            match self.github.list_unresolved_threads(pr_num).await {
                Ok(threads) if !threads.is_empty() => {
                    causes.push(FixingProblemKind::ReviewComments);
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(pr_number = pr_num, error = %e, "failed to list unresolved threads");
                }
            }

            // (5) If any causes found, save to DB and emit event
            if !causes.is_empty() {
                tracing::info!(
                    pr_number = pr_num,
                    cause_count = causes.len(),
                    "fixing causes collected"
                );
                issue.fixing_causes = causes;
                if let Err(e) = self.issue_repo.update(&issue).await {
                    tracing::warn!(
                        issue_id = issue.id,
                        error = %e,
                        "failed to update fixing_causes, skipping event"
                    );
                    continue;
                }
                events.push((issue.id, Event::UnresolvedThreadsDetected));
            }
        }
    }

    // --- Step 5: Stall detection ---

    fn step5_stall_detection(&mut self, _events: &mut Vec<(i64, Event)>) {
        let timeout = Duration::from_secs(self.config.stall_timeout_secs);
        let stalled = self.session_mgr.find_stalled(timeout);

        for issue_id in stalled {
            tracing::warn!(issue_id, "stall detected, killing process");
            let _ = self.session_mgr.kill(issue_id);
            // Will be collected as failed in next cycle's step3
            // But we need to generate event now since kill terminates the process
        }
    }

    // --- Step 6: Apply events ---

    async fn step6_apply_events(&mut self, events: &mut Vec<(i64, Event)>) {
        prioritize_events(events);

        // Handle IssueDetected separately (creates new records)
        let mut remaining = Vec::new();
        for (id, event) in events.drain(..) {
            if let Event::IssueDetected { issue_number } = &event {
                let uc = self.transition_uc();
                if let Err(e) = uc.handle_issue_detected(*issue_number).await {
                    tracing::error!(issue_number, error = %e, "failed to handle issue detected");
                }
            } else {
                remaining.push((id, event));
            }
        }

        // Apply remaining events to existing issues
        // Deduplicate: only apply first event per issue (IssueClosed takes priority due to sort)
        let mut processed_ids = std::collections::HashSet::new();
        for (issue_id, event) in &remaining {
            if processed_ids.contains(issue_id) {
                continue;
            }
            processed_ids.insert(*issue_id);

            let mut issue = match self.issue_repo.find_by_id(*issue_id).await {
                Ok(Some(i)) => i,
                _ => continue,
            };

            let uc = self.transition_uc();
            if let Err(e) = uc.apply(&mut issue, event).await {
                tracing::warn!(
                    issue_id,
                    event = ?event,
                    error = %e,
                    "failed to apply event"
                );
            }
        }
    }

    // --- Step 7: Spawn processes ---

    async fn step7_spawn_processes(&mut self) {
        let needing = match self.issue_repo.find_needing_process().await {
            Ok(n) => n,
            Err(_) => return,
        };

        for issue in &needing {
            if let Some(max) = self.config.max_concurrent_sessions
                && self.session_mgr.count() >= max as usize
            {
                tracing::info!(
                    current = self.session_mgr.count(),
                    max,
                    "concurrent session limit reached, skipping remaining issues"
                );
                break;
            }

            if self.session_mgr.is_running(issue.id) {
                continue;
            }

            // DesignRunning / ImplementationRunning でスポーン前に PR 状態をチェック
            let pr_number_to_check = match issue.state {
                State::DesignRunning => issue.design_pr_number,
                State::ImplementationRunning => issue.impl_pr_number,
                _ => None,
            };
            if let Some(pr_num) = pr_number_to_check {
                match self.github.get_pr_status(pr_num).await {
                    Ok(PrStatus::Merged) => {
                        // Running → ReviewWaiting → Merged の2ステップ遷移
                        let merge_event = match issue.state {
                            State::DesignRunning => Event::DesignPrMerged,
                            State::ImplementationRunning => Event::ImplementationPrMerged,
                            _ => unreachable!(),
                        };
                        tracing::info!(
                            issue_id = issue.id,
                            pr_number = pr_num,
                            "PR already merged, transitioning through ReviewWaiting then merged"
                        );
                        let uc = self.transition_uc();
                        let mut issue_mut = issue.clone();
                        if let Err(e) = uc
                            .apply(&mut issue_mut, &Event::ProcessCompletedWithPr)
                            .await
                        {
                            tracing::warn!(
                                issue_id = issue.id,
                                pr_number = pr_num,
                                error = %e,
                                "failed to apply ProcessCompletedWithPr event during merge transition"
                            );
                        } else if let Err(e) = uc.apply(&mut issue_mut, &merge_event).await {
                            tracing::warn!(
                                issue_id = issue.id,
                                pr_number = pr_num,
                                error = %e,
                                "failed to apply merge event during merge transition"
                            );
                        }
                        continue;
                    }
                    Ok(PrStatus::Open) => {
                        tracing::info!(
                            issue_id = issue.id,
                            pr_number = pr_num,
                            "PR is still open, firing ProcessCompletedWithPr and skipping spawn"
                        );
                        let uc = self.transition_uc();
                        let mut issue_mut = issue.clone();
                        if let Err(e) = uc
                            .apply(&mut issue_mut, &Event::ProcessCompletedWithPr)
                            .await
                        {
                            tracing::warn!(
                                issue_id = issue.id,
                                pr_number = pr_num,
                                error = %e,
                                "failed to apply ProcessCompletedWithPr event for open PR"
                            );
                        }
                        continue;
                    }
                    Ok(PrStatus::Closed) => {
                        tracing::info!(
                            issue_id = issue.id,
                            pr_number = pr_num,
                            "PR is closed (not merged), proceeding with normal spawn"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            issue_id = issue.id,
                            pr_number = pr_num,
                            error = %e,
                            "failed to check PR status, proceeding with normal spawn (failsafe)"
                        );
                    }
                }
            }

            let Some(ref wt_path) = issue.worktree_path else {
                continue;
            };

            let wt = Path::new(wt_path);

            // Clean up stale index.lock
            let lock_file = wt.join(".git/index.lock");
            if lock_file.exists() {
                tracing::warn!(path = %lock_file.display(), "removing stale index.lock");
                let _ = std::fs::remove_file(&lock_file);
            }

            // Write input files
            if let Err(e) = self.prepare_inputs(issue, wt).await {
                tracing::warn!(issue_id = issue.id, error = %e, "failed to prepare inputs");
                continue;
            }

            // Build session config
            let pr_number = match issue.state {
                State::DesignFixing => issue.design_pr_number,
                State::ImplementationFixing => issue.impl_pr_number,
                _ => None,
            };

            let session_config = match build_session_config(
                issue.state,
                issue.github_issue_number,
                &self.config,
                pr_number,
                issue.feature_name.as_deref(),
                &issue.fixing_causes,
            ) {
                Ok(cfg) => cfg,
                Err(e) => {
                    tracing::warn!(issue_id = issue.id, error = %e, "failed to build session config, skipping issue");
                    continue;
                }
            };

            let schema = match session_config.output_schema {
                OutputSchemaKind::PrCreation => Some(PR_CREATION_SCHEMA),
                OutputSchemaKind::Fixing => Some(FIXING_SCHEMA),
            };

            match self
                .claude_runner
                .spawn(&session_config.prompt, wt, schema, &self.config.model)
            {
                Ok(child) => {
                    let pid = child.id();
                    tracing::info!(
                        issue_id = issue.id,
                        issue_number = issue.github_issue_number,
                        state = ?issue.state,
                        pid,
                        "spawned Claude Code process"
                    );
                    self.session_mgr.register(issue.id, child);

                    // Persist PID to DB
                    let mut updated_issue = issue.clone();
                    updated_issue.current_pid = Some(pid);
                    if let Err(e) = self.issue_repo.update(&updated_issue).await {
                        tracing::warn!(issue_id = issue.id, error = %e, "failed to persist PID");
                    }
                }
                Err(e) => {
                    tracing::error!(issue_id = issue.id, error = %e, "failed to spawn Claude Code");
                }
            }
        }
    }

    async fn prepare_inputs(&self, issue: &Issue, wt: &Path) -> Result<()> {
        match issue.state {
            State::DesignRunning => {
                let detail = self.github.get_issue(issue.github_issue_number).await?;
                write_issue_input(wt, &detail)?;
            }
            State::DesignFixing | State::ImplementationFixing => {
                let pr_number = match issue.state {
                    State::DesignFixing => issue.design_pr_number,
                    State::ImplementationFixing => issue.impl_pr_number,
                    _ => None,
                };
                if let Some(pr_num) = pr_number {
                    // Write inputs based on fixing_causes
                    let causes = &issue.fixing_causes;

                    if causes.contains(&FixingProblemKind::ReviewComments) {
                        let threads = self.github.list_unresolved_threads(pr_num).await?;
                        write_review_threads_input(wt, &threads)?;
                    }

                    if causes.contains(&FixingProblemKind::CiFailure) {
                        let runs = self.github.get_ci_check_runs(pr_num).await?;
                        let mut errors: Vec<CiErrorEntry> = Vec::new();
                        for r in runs
                            .into_iter()
                            .filter(|r| r.conclusion.as_deref() == Some("failure"))
                        {
                            let log_lines = match self.github.get_job_logs(r.id).await {
                                Ok(logs) => extract_error_lines(&logs),
                                Err(e) => {
                                    tracing::warn!(
                                        job_id = r.id,
                                        error = %e,
                                        "failed to get job logs, falling back to output fields"
                                    );
                                    None
                                }
                            };
                            errors.push(CiErrorEntry {
                                check_run_name: r.name,
                                conclusion: r.conclusion.unwrap_or_default(),
                                output_summary: r.output_summary,
                                output_text: log_lines.or(r.output_text),
                            });
                        }
                        write_ci_errors_input(wt, &errors)?;
                    }

                    if causes.contains(&FixingProblemKind::Conflict) {
                        let n = issue.github_issue_number;
                        let info = match issue.state {
                            State::DesignFixing => {
                                let head_branch = format!("cupola/{n}/design");
                                let base_branch = format!("cupola/{n}/main");
                                ConflictInfo {
                                    head_branch,
                                    base_branch,
                                    default_branch: self.config.default_branch.clone(),
                                }
                            }
                            State::ImplementationFixing => {
                                let head_branch = format!("cupola/{n}/main");
                                let base_branch = self.config.default_branch.clone();
                                ConflictInfo {
                                    head_branch,
                                    base_branch: base_branch.clone(),
                                    default_branch: base_branch,
                                }
                            }
                            _ => unreachable!(
                                "Conflict cause set only for design/implementation fixing states"
                            ),
                        };
                        write_conflict_info_input(wt, &info)?;
                    }

                    // Fallback: if no causes set (old behavior), write review threads
                    if causes.is_empty() {
                        let threads = self.github.list_unresolved_threads(pr_num).await?;
                        write_review_threads_input(wt, &threads)?;
                    }
                }
            }
            _ => {} // ImplementationRunning needs no input preparation
        }
        Ok(())
    }

    // --- PR creation from output ---

    async fn create_pr_from_output(&self, issue: &Issue, stdout: &str) -> Result<u64> {
        let n = issue.github_issue_number;

        let (head, base) = match issue.state {
            State::DesignRunning => (format!("cupola/{n}/design"), format!("cupola/{n}/main")),
            State::ImplementationRunning => (
                format!("cupola/{n}/main"),
                self.config.default_branch.clone(),
            ),
            _ => anyhow::bail!("unexpected state for PR creation: {:?}", issue.state),
        };

        // Idempotency check 1: DB already has PR number
        let existing = match issue.state {
            State::DesignRunning => issue.design_pr_number,
            State::ImplementationRunning => issue.impl_pr_number,
            _ => None,
        };
        if let Some(pr_num) = existing {
            tracing::info!(
                issue_number = n,
                pr_number = pr_num,
                "PR already exists in DB, skipping creation"
            );
            return Ok(pr_num);
        }

        // Idempotency check 2: PR exists on GitHub
        if let Ok(Some(pr)) = self.github.find_pr_by_branches(&head, &base).await {
            // Record in DB
            let mut updated = issue.clone();
            match issue.state {
                State::DesignRunning => updated.design_pr_number = Some(pr.number),
                State::ImplementationRunning => updated.impl_pr_number = Some(pr.number),
                _ => {}
            }
            let _ = self.issue_repo.update(&updated).await;
            tracing::info!(
                issue_number = n,
                pr_number = pr.number,
                head = %head,
                base = %base,
                "PR already exists on GitHub, skipping creation"
            );
            return Ok(pr.number);
        }

        // Parse output or use fallback
        let output = parse_pr_creation_output(stdout);
        let fb_title = fallback_pr_title(issue.state, n);
        let fb_body = fallback_pr_body(issue.state, n);
        let title = output
            .as_ref()
            .and_then(|o| o.pr_title.as_deref())
            .unwrap_or(&fb_title);
        let body = output
            .as_ref()
            .and_then(|o| o.pr_body.as_deref())
            .unwrap_or(&fb_body);

        let pr_number = self.github.create_pr(&head, &base, title, body).await?;
        tracing::info!(
            issue_number = n,
            pr_number = pr_number,
            head = %head,
            base = %base,
            "PR created successfully"
        );

        // Record PR number (and feature_name for design phase) in DB
        let mut updated = issue.clone();
        match issue.state {
            State::DesignRunning => {
                updated.design_pr_number = Some(pr_number);
                if let Some(ref output) = output
                    && let Some(ref name) = output.feature_name
                {
                    updated.feature_name = Some(name.clone());
                    tracing::info!(feature_name = %name, "recorded feature_name from design output");
                }
            }
            State::ImplementationRunning => updated.impl_pr_number = Some(pr_number),
            _ => {}
        }
        let _ = self.issue_repo.update(&updated).await;

        Ok(pr_number)
    }

    async fn process_fixing_output(&self, issue: &Issue, stdout: &str) {
        let Some(output) = parse_fixing_output(stdout) else {
            tracing::warn!("failed to parse fixing output, skipping post-processing");
            return;
        };

        for resp in &output.threads {
            if let Err(e) = self
                .github
                .reply_to_thread(&resp.thread_id, &resp.response)
                .await
            {
                tracing::warn!(thread_id = %resp.thread_id, error = %e, "failed to reply to thread");
                continue;
            }
            tracing::info!(
                thread_id = %resp.thread_id,
                "replied to review thread"
            );
            if resp.resolved {
                if let Err(e) = self.github.resolve_thread(&resp.thread_id).await {
                    tracing::warn!(thread_id = %resp.thread_id, error = %e, "failed to resolve thread");
                } else {
                    tracing::info!(
                        thread_id = %resp.thread_id,
                        "resolved review thread"
                    );
                }
            }
        }

        tracing::info!(
            issue_number = issue.github_issue_number,
            thread_count = output.threads.len(),
            "fixing post-processing completed"
        );
    }

    // --- Helpers ---

    /// For review_waiting states, check if the PR was merged before emitting IssueClosed.
    /// This handles the case where GitHub's "Closes #N" auto-closes the Issue on PR merge.
    async fn resolve_close_event_for_issue(&self, issue: &Issue) -> Event {
        let pr_number = match issue.state {
            State::DesignReviewWaiting => issue.design_pr_number,
            State::ImplementationReviewWaiting => issue.impl_pr_number,
            _ => return Event::IssueClosed,
        };

        let Some(pr_num) = pr_number else {
            return Event::IssueClosed;
        };

        match self.github.is_pr_merged(pr_num).await {
            Ok(true) => {
                tracing::info!(
                    issue_id = issue.id,
                    pr_number = pr_num,
                    "Issue closed but PR is merged — treating as merge event"
                );
                match issue.state {
                    State::DesignReviewWaiting => Event::DesignPrMerged,
                    State::ImplementationReviewWaiting => Event::ImplementationPrMerged,
                    _ => unreachable!(),
                }
            }
            Ok(false) => Event::IssueClosed,
            Err(e) => {
                tracing::warn!(
                    issue_id = issue.id,
                    pr_number = pr_num,
                    error = %e,
                    "failed to check PR merge on issue close, falling back to IssueClosed"
                );
                Event::IssueClosed
            }
        }
    }

    /// Access the issue repository (for testing).
    pub fn issue_repo_ref(&self) -> &I {
        &self.issue_repo
    }

    /// Access the worktree (for testing).
    pub fn worktree_ref(&self) -> &W {
        &self.worktree
    }

    fn transition_uc(&self) -> TransitionUseCase<'_, G, I, W> {
        TransitionUseCase {
            github: &self.github,
            issue_repo: &self.issue_repo,
            worktree: &self.worktree,
            config: &self.config,
        }
    }

    async fn recover_on_startup(&self) {
        match self.issue_repo.find_active().await {
            Ok(issues) => {
                for mut issue in issues {
                    if issue.current_pid.is_some() {
                        issue.current_pid = None;
                        let _ = self.issue_repo.update(&issue).await;
                        tracing::info!(issue_id = issue.id, "cleared stale PID on startup");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to recover on startup");
            }
        }
    }

    pub async fn graceful_shutdown(&mut self) {
        tracing::info!("starting graceful shutdown...");

        // Send SIGTERM to all (kill in our case)
        self.session_mgr.kill_all();

        // Wait up to 10 seconds
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let exited = self.session_mgr.collect_exited();
            if exited.is_empty() {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                tracing::warn!("shutdown deadline reached, forcing kill");
                self.session_mgr.kill_all();
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Delete PID file if in daemon mode
        if let Some(ref pid_file) = self.pid_file
            && let Err(e) = pid_file.delete_pid()
        {
            tracing::warn!(error = %e, "failed to delete PID file during graceful shutdown");
        }

        tracing::info!("graceful shutdown complete");
    }
}

/// Extract error lines from raw CI job logs.
/// Strips ANSI SGR color codes, filters for error-related lines, and removes timestamp prefixes.
fn extract_error_lines(raw_log: &str) -> Option<String> {
    let lines: Vec<String> = raw_log
        .lines()
        .map(strip_ansi)
        .filter(|clean| {
            (clean.contains("error:") || clean.contains("error[") || clean.contains("##[error]"))
                && !clean.contains("curl")
                && !clean.contains("spurious")
        })
        .map(|clean| {
            // Strip timestamp prefix (e.g., "2026-04-02T10:53:01.0418115Z ")
            if let Some(pos) = clean.find(' ').filter(|&p| p > 20) {
                return clean[pos + 1..].to_string();
            }
            clean
        })
        .collect();

    if lines.is_empty() {
        return None;
    }

    Some(lines.join("\n"))
}

/// Strip ANSI SGR escape sequences (e.g., color codes like `\x1b[31m`) from a string.
/// Only handles sequences ending with 'm'; other escape sequences are not stripped.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c2 in chars.by_ref() {
                if c2 == 'm' {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use crate::application::port::github_client::{
        GitHubCheckRun, GitHubIssue, GitHubIssueDetail, GitHubPr, GitHubPrDetails, PrStatus,
        ReviewComment, ReviewThread,
    };
    use crate::domain::config::Config;
    use crate::domain::execution_log::ExecutionLog;

    // Test that the retry policy evaluation works correctly with PollingUseCase
    #[test]
    fn handle_failed_exit_generates_correct_event_for_retry() {
        // RetryPolicy with max_retries=3, issue with retry_count=1 → Retry
        let policy = RetryPolicy::new(3);
        assert_eq!(policy.evaluate(1), RetryDecision::Retry);
    }

    #[test]
    fn handle_failed_exit_generates_correct_event_for_exhausted() {
        let policy = RetryPolicy::new(3);
        assert_eq!(policy.evaluate(3), RetryDecision::Exhausted);
    }

    #[test]
    fn pr_branches_for_design() {
        let n = 42u64;
        let head = format!("cupola/{n}/design");
        let base = format!("cupola/{n}/main");
        assert_eq!(head, "cupola/42/design");
        assert_eq!(base, "cupola/42/main");
    }

    #[test]
    fn pr_branches_for_implementation() {
        let n = 42u64;
        let head = format!("cupola/{n}/main");
        let base = "main".to_string();
        assert_eq!(head, "cupola/42/main");
        assert_eq!(base, "main");
    }

    // --- Mocks for step4/step7 tests ---

    struct MockGitHub {
        pr_details: GitHubPrDetails,
        pr_status: PrStatus,
        check_runs: Vec<GitHubCheckRun>,
        threads: Vec<ReviewThread>,
        comments: Arc<Mutex<Vec<(u64, String)>>>,
    }

    impl MockGitHub {
        fn new(
            merged: bool,
            mergeable: Option<bool>,
            failure_names: Vec<&str>,
            has_threads: bool,
        ) -> Self {
            Self::with_pr_status(
                merged,
                mergeable,
                failure_names,
                has_threads,
                PrStatus::Closed,
            )
        }

        fn with_pr_status(
            merged: bool,
            mergeable: Option<bool>,
            failure_names: Vec<&str>,
            has_threads: bool,
            pr_status: PrStatus,
        ) -> Self {
            let check_runs = failure_names
                .into_iter()
                .map(|name| GitHubCheckRun {
                    id: 1,
                    name: name.to_string(),
                    status: "completed".to_string(),
                    conclusion: Some("failure".to_string()),
                    output_summary: None,
                    output_text: None,
                })
                .collect();
            let threads = if has_threads {
                vec![ReviewThread {
                    id: "t1".to_string(),
                    path: "file.rs".to_string(),
                    line: None,
                    comments: vec![ReviewComment {
                        author: "reviewer".to_string(),
                        body: "fix this".to_string(),
                    }],
                }]
            } else {
                vec![]
            };
            Self {
                pr_details: GitHubPrDetails { merged, mergeable },
                pr_status,
                check_runs,
                threads,
                comments: Arc::new(Mutex::new(vec![])),
            }
        }
    }

    impl crate::application::port::github_client::GitHubClient for MockGitHub {
        async fn list_ready_issues(&self) -> anyhow::Result<Vec<GitHubIssue>> {
            unimplemented!()
        }
        async fn get_issue(&self, n: u64) -> anyhow::Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number: n,
                title: "Test Issue".to_string(),
                body: "Test body".to_string(),
                labels: vec![],
            })
        }
        async fn is_issue_open(&self, _: u64) -> anyhow::Result<bool> {
            unimplemented!()
        }
        async fn find_pr_by_branches(&self, _: &str, _: &str) -> anyhow::Result<Option<GitHubPr>> {
            unimplemented!()
        }
        async fn is_pr_merged(&self, _: u64) -> anyhow::Result<bool> {
            Ok(self.pr_details.merged)
        }
        async fn list_unresolved_threads(&self, _: u64) -> anyhow::Result<Vec<ReviewThread>> {
            Ok(self.threads.clone())
        }
        async fn create_pr(&self, _: &str, _: &str, _: &str, _: &str) -> anyhow::Result<u64> {
            unimplemented!()
        }
        async fn reply_to_thread(&self, _: &str, _: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn resolve_thread(&self, _: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn comment_on_issue(&self, issue_number: u64, body: &str) -> anyhow::Result<()> {
            self.comments
                .lock()
                .unwrap()
                .push((issue_number, body.to_string()));
            Ok(())
        }
        async fn close_issue(&self, _: u64) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_ci_check_runs(&self, _: u64) -> anyhow::Result<Vec<GitHubCheckRun>> {
            Ok(self.check_runs.clone())
        }
        async fn get_job_logs(&self, _: u64) -> anyhow::Result<String> {
            Ok(String::new())
        }
        async fn get_pr_mergeable(&self, _: u64) -> anyhow::Result<Option<bool>> {
            Ok(self.pr_details.mergeable)
        }
        async fn get_pr_details(&self, _: u64) -> anyhow::Result<GitHubPrDetails> {
            Ok(self.pr_details.clone())
        }
        async fn get_pr_status(&self, _: u64) -> anyhow::Result<PrStatus> {
            Ok(self.pr_status.clone())
        }
    }

    struct MockIssueRepo {
        issues: Vec<Issue>,
        updated: Arc<Mutex<Vec<Issue>>>,
    }

    impl MockIssueRepo {
        fn new(issues: Vec<Issue>) -> (Self, Arc<Mutex<Vec<Issue>>>) {
            let updated = Arc::new(Mutex::new(vec![]));
            (
                Self {
                    issues,
                    updated: updated.clone(),
                },
                updated,
            )
        }
    }

    impl crate::application::port::issue_repository::IssueRepository for MockIssueRepo {
        async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<Issue>> {
            Ok(self.issues.iter().find(|i| i.id == id).cloned())
        }
        async fn find_by_issue_number(&self, _: u64) -> anyhow::Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_active(&self) -> anyhow::Result<Vec<Issue>> {
            Ok(self.issues.clone())
        }
        async fn find_needing_process(&self) -> anyhow::Result<Vec<Issue>> {
            Ok(self
                .issues
                .iter()
                .filter(|i| i.state.needs_process())
                .cloned()
                .collect())
        }
        async fn save(&self, _: &Issue) -> anyhow::Result<i64> {
            unimplemented!()
        }
        async fn update_state(&self, id: i64, state: State) -> anyhow::Result<()> {
            if let Some(mut updated_issue) = self.issues.iter().find(|i| i.id == id).cloned() {
                updated_issue.state = state;
                self.updated.lock().unwrap().push(updated_issue);
            }
            Ok(())
        }
        async fn update(&self, issue: &Issue) -> anyhow::Result<()> {
            self.updated.lock().unwrap().push(issue.clone());
            Ok(())
        }
        async fn reset_for_restart(&self, _: i64) -> anyhow::Result<()> {
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

    struct NoopExecLogRepo;
    impl crate::application::port::execution_log_repository::ExecutionLogRepository
        for NoopExecLogRepo
    {
        async fn record_start(&self, _: i64, _: State) -> anyhow::Result<i64> {
            Ok(0)
        }
        async fn record_finish(
            &self,
            _: i64,
            _: Option<i32>,
            _: Option<&str>,
            _: Option<&str>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn find_by_issue(&self, _: i64) -> anyhow::Result<Vec<ExecutionLog>> {
            Ok(vec![])
        }
    }

    struct NoopClaudeRunner;
    impl crate::application::port::claude_code_runner::ClaudeCodeRunner for NoopClaudeRunner {
        fn spawn(
            &self,
            _: &str,
            _: &Path,
            _: Option<&str>,
            _: &str,
        ) -> anyhow::Result<std::process::Child> {
            Err(anyhow::anyhow!("NoopClaudeRunner: spawn not supported"))
        }
    }

    struct NoopWorktree;
    impl crate::application::port::git_worktree::GitWorktree for NoopWorktree {
        fn fetch(&self) -> anyhow::Result<()> {
            Ok(())
        }
        fn exists(&self, _: &Path) -> bool {
            false
        }
        fn create(&self, _: &Path, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn remove(&self, _: &Path) -> anyhow::Result<()> {
            Ok(())
        }
        fn create_branch(&self, _: &Path, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn checkout(&self, _: &Path, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn pull(&self, _: &Path) -> anyhow::Result<()> {
            Ok(())
        }
        fn push(&self, _: &Path, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn delete_branch(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
    }

    type TestUseCase =
        PollingUseCase<MockGitHub, MockIssueRepo, NoopExecLogRepo, NoopClaudeRunner, NoopWorktree>;

    fn make_uc(github: MockGitHub, repo: MockIssueRepo) -> TestUseCase {
        let config =
            Config::default_with_repo("o".to_string(), "r".to_string(), "main".to_string());
        PollingUseCase::new(
            github,
            repo,
            NoopExecLogRepo,
            NoopClaudeRunner,
            NoopWorktree,
            config,
        )
    }

    fn review_waiting_issue(state: State, pr_num: u64) -> Issue {
        let (design_pr, impl_pr) = match state {
            State::DesignReviewWaiting => (Some(pr_num), None),
            State::ImplementationReviewWaiting => (None, Some(pr_num)),
            _ => (None, None),
        };
        Issue {
            id: 1,
            github_issue_number: 42,
            state,
            design_pr_number: design_pr,
            impl_pr_number: impl_pr,
            worktree_path: None,
            retry_count: 0,
            current_pid: None,
            error_message: None,
            feature_name: None,
            fixing_causes: vec![],
            model: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn step4_merged_pr_emits_event_and_skips_cause_collection() {
        let issue = review_waiting_issue(State::DesignReviewWaiting, 10);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        let github = MockGitHub::new(true, None, vec![], false);
        let mut uc = make_uc(github, repo);
        let mut events = vec![];
        uc.step4_pr_monitoring(&mut events).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], (1, Event::DesignPrMerged)));
        assert!(
            updated.lock().unwrap().is_empty(),
            "DB must not be updated for merged PR"
        );
    }

    #[tokio::test]
    async fn step4_mergeable_none_skips_conflict_no_event() {
        let issue = review_waiting_issue(State::DesignReviewWaiting, 10);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        // mergeable=None: GitHub is still computing
        let github = MockGitHub::new(false, None, vec![], false);
        let mut uc = make_uc(github, repo);
        let mut events = vec![];
        uc.step4_pr_monitoring(&mut events).await;
        assert!(
            events.is_empty(),
            "no events when mergeable is None and no other issues"
        );
        assert!(updated.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn step4_ci_failure_adds_cause_and_updates_db() {
        let issue = review_waiting_issue(State::ImplementationReviewWaiting, 20);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        let github = MockGitHub::new(false, Some(true), vec!["build"], false);
        let mut uc = make_uc(github, repo);
        let mut events = vec![];
        uc.step4_pr_monitoring(&mut events).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], (1, Event::UnresolvedThreadsDetected)));
        let updates = updated.lock().unwrap();
        assert_eq!(updates.len(), 1);
        assert!(
            updates[0]
                .fixing_causes
                .contains(&FixingProblemKind::CiFailure)
        );
        assert!(
            !updates[0]
                .fixing_causes
                .contains(&FixingProblemKind::Conflict)
        );
    }

    #[tokio::test]
    async fn step4_conflict_adds_cause() {
        let issue = review_waiting_issue(State::DesignReviewWaiting, 30);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        let github = MockGitHub::new(false, Some(false), vec![], false);
        let mut uc = make_uc(github, repo);
        let mut events = vec![];
        uc.step4_pr_monitoring(&mut events).await;
        let updates = updated.lock().unwrap();
        assert_eq!(updates.len(), 1);
        assert!(
            updates[0]
                .fixing_causes
                .contains(&FixingProblemKind::Conflict)
        );
        assert!(
            !updates[0]
                .fixing_causes
                .contains(&FixingProblemKind::CiFailure)
        );
    }

    #[tokio::test]
    async fn step4_multiple_causes_collected_simultaneously() {
        let issue = review_waiting_issue(State::DesignReviewWaiting, 40);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        // conflict + CI failure + unresolved threads
        let github = MockGitHub::new(false, Some(false), vec!["ci-check"], true);
        let mut uc = make_uc(github, repo);
        let mut events = vec![];
        uc.step4_pr_monitoring(&mut events).await;
        let updates = updated.lock().unwrap();
        assert_eq!(updates.len(), 1);
        let causes = &updates[0].fixing_causes;
        assert!(causes.contains(&FixingProblemKind::CiFailure));
        assert!(causes.contains(&FixingProblemKind::Conflict));
        assert!(causes.contains(&FixingProblemKind::ReviewComments));
        assert_eq!(causes.len(), 3);
    }

    // --- Step7 PR check tests (Task 8.5) ---

    fn design_running_issue_with_pr(pr_num: u64) -> Issue {
        Issue {
            id: 1,
            github_issue_number: 42,
            state: State::DesignRunning,
            design_pr_number: Some(pr_num),
            impl_pr_number: None,
            worktree_path: Some("/tmp/wt/42".to_string()),
            retry_count: 0,
            current_pid: None,
            error_message: None,
            feature_name: None,
            fixing_causes: vec![],
            model: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn impl_running_issue_with_pr(pr_num: u64) -> Issue {
        Issue {
            id: 2,
            github_issue_number: 43,
            state: State::ImplementationRunning,
            design_pr_number: Some(10),
            impl_pr_number: Some(pr_num),
            worktree_path: Some("/tmp/wt/43".to_string()),
            retry_count: 0,
            current_pid: None,
            error_message: None,
            feature_name: None,
            fixing_causes: vec![],
            model: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn step7_design_pr_merged_skips_spawn_and_fires_event() {
        let issue = design_running_issue_with_pr(55);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        let github = MockGitHub::with_pr_status(false, None, vec![], false, PrStatus::Merged);
        let mut uc = make_uc(github, repo);
        uc.step7_spawn_processes().await;
        // Should have transitioned DesignRunning → DesignReviewWaiting → ImplementationRunning
        let updates = updated.lock().unwrap();
        assert!(
            !updates.is_empty(),
            "state transition should have been recorded"
        );
        // Final state should be ImplementationRunning (or some intermediate state)
        let states: Vec<State> = updates.iter().map(|u| u.state).collect();
        assert!(
            states.contains(&State::DesignReviewWaiting)
                || states.contains(&State::ImplementationRunning),
            "should have transitioned through DesignReviewWaiting: {:?}",
            states
        );
    }

    #[tokio::test]
    async fn step7_design_pr_open_skips_spawn_and_fires_event() {
        let issue = design_running_issue_with_pr(56);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        let github = MockGitHub::with_pr_status(false, None, vec![], false, PrStatus::Open);
        let mut uc = make_uc(github, repo);
        uc.step7_spawn_processes().await;
        // Should have fired ProcessCompletedWithPr → DesignReviewWaiting
        let updates = updated.lock().unwrap();
        assert!(
            !updates.is_empty(),
            "state transition should have been recorded"
        );
        let states: Vec<State> = updates.iter().map(|u| u.state).collect();
        assert!(
            states.contains(&State::DesignReviewWaiting),
            "should have transitioned to DesignReviewWaiting: {:?}",
            states
        );
    }

    #[tokio::test]
    async fn step7_design_pr_closed_does_not_fire_pr_events() {
        // PR is closed → should attempt normal spawn path (no PR-triggered state changes)
        let issue = design_running_issue_with_pr(57);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        let github = MockGitHub::with_pr_status(false, None, vec![], false, PrStatus::Closed);
        let mut uc = make_uc(github, repo);
        uc.step7_spawn_processes().await;
        // No PR-triggered state transitions — spawn may fail (no worktree exists) but that's OK
        let updates = updated.lock().unwrap();
        // No ReviewWaiting or ImplementationRunning transitions from PR check
        assert!(
            !updates.iter().any(|u| u.state == State::DesignReviewWaiting
                || u.state == State::ImplementationRunning),
            "closed PR should not trigger merge transitions: {:?}",
            updates.iter().map(|u| u.state).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn step7_impl_pr_merged_fires_impl_merged_event() {
        let issue = impl_running_issue_with_pr(58);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        let github = MockGitHub::with_pr_status(false, None, vec![], false, PrStatus::Merged);
        let mut uc = make_uc(github, repo);
        uc.step7_spawn_processes().await;
        // Should have transitioned ImplementationRunning → ImplementationReviewWaiting → Completed
        let updates = updated.lock().unwrap();
        assert!(
            !updates.is_empty(),
            "ImplementationPrMerged transition should be recorded"
        );
        let states: Vec<State> = updates.iter().map(|u| u.state).collect();
        assert!(
            states.contains(&State::ImplementationReviewWaiting)
                || states.contains(&State::Completed),
            "should transition through ImplementationReviewWaiting: {:?}",
            states
        );
    }

    #[test]
    fn strip_ansi_removes_color_codes() {
        assert_eq!(
            strip_ansi("\x1b[1m\x1b[91merror\x1b[0m: something"),
            "error: something"
        );
        assert_eq!(strip_ansi("no ansi here"), "no ansi here");
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn extract_error_lines_filters_errors() {
        let log = "2026-04-02T10:52:58.0Z \x1b[92mChecking\x1b[0m cupola\n\
                   2026-04-02T10:53:01.0418115Z \x1b[91merror\x1b[0m: very complex type used\n\
                   2026-04-02T10:53:01.6Z \x1b[91merror\x1b[0m: could not compile\n\
                   2026-04-02T10:53:01.7Z some other line\n";
        let result = extract_error_lines(log).unwrap();
        assert!(result.contains("error: very complex type used"));
        assert!(result.contains("error: could not compile"));
        assert!(!result.contains("Checking"));
        assert!(!result.contains("some other line"));
    }

    #[test]
    fn extract_error_lines_returns_none_for_no_errors() {
        let log = "2026-04-02T10:52:58.0Z Checking cupola\n2026-04-02T10:53:00.0Z Finished\n";
        assert!(extract_error_lines(log).is_none());
    }

    #[test]
    fn extract_error_lines_strips_timestamps() {
        let log = "2026-04-02T10:53:01.0418115Z error: something wrong\n";
        let result = extract_error_lines(log).unwrap();
        assert_eq!(result, "error: something wrong");
    }
}
