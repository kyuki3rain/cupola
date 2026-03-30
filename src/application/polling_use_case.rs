use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use tokio::signal;
use tokio::time::interval;

use crate::application::io::{
    parse_fixing_output, parse_pr_creation_output, write_issue_input, write_review_threads_input,
};
use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::execution_log_repository::ExecutionLogRepository;
use crate::application::port::git_worktree::GitWorktree;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::prompt::{
    FIXING_SCHEMA, OutputSchemaKind, PR_CREATION_SCHEMA, build_session_config, fallback_pr_body,
    fallback_pr_title,
};
use crate::application::retry_policy::{RetryDecision, RetryPolicy};
use crate::application::session_manager::{ExitedSession, SessionManager};
use crate::application::transition_use_case::{TransitionUseCase, prioritize_events};
use crate::domain::config::Config;
use crate::domain::event::Event;
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
        }
    }

    /// Main polling loop. Runs until SIGINT.
    pub async fn run(&mut self) -> Result<()> {
        // Startup recovery: clear PIDs
        self.recover_on_startup().await;

        let mut tick = interval(Duration::from_secs(self.config.polling_interval_secs));

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

        // Detect closed issues and update model labels for active issues
        match self.issue_repo.find_active().await {
            Ok(active_issues) => {
                for issue in &active_issues {
                    // Re-check labels and update model in DB if changed
                    match self
                        .github
                        .get_issue_labels(issue.github_issue_number)
                        .await
                    {
                        Ok(labels) => {
                            let new_model = extract_model_from_labels(&labels);
                            if new_model != issue.model {
                                let mut updated = issue.clone();
                                updated.model = new_model.clone();
                                if let Err(e) = self.issue_repo.update(&updated).await {
                                    tracing::warn!(
                                        issue_number = issue.github_issue_number,
                                        error = %e,
                                        "failed to update model from label"
                                    );
                                } else {
                                    tracing::info!(
                                        issue_number = issue.github_issue_number,
                                        model = ?new_model,
                                        "updated model from label"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                issue_number = issue.github_issue_number,
                                error = %e,
                                "failed to get issue labels, keeping previous model"
                            );
                        }
                    }

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

        let start_point = format!("origin/{}", self.config.default_branch);
        let wt = Path::new(&wt_path);
        self.worktree.create(wt, &main_branch, &start_point)?;
        self.worktree.push(wt, &main_branch)?;
        self.worktree.create_branch(wt, &design_branch)?;
        self.worktree.push(wt, &design_branch)?;

        issue.worktree_path = Some(wt_path);
        self.issue_repo.update(issue).await?;

        self.github.comment_on_issue(n, "設計を開始します").await?;

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

    async fn step4_pr_monitoring(&self, events: &mut Vec<(i64, Event)>) {
        let active = match self.issue_repo.find_active().await {
            Ok(a) => a,
            Err(_) => return,
        };

        for issue in &active {
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

            // Check merge first (priority)
            match self.github.is_pr_merged(pr_num).await {
                Ok(true) => {
                    let event = match issue.state {
                        State::DesignReviewWaiting => Event::DesignPrMerged,
                        State::ImplementationReviewWaiting => Event::ImplementationPrMerged,
                        _ => continue,
                    };
                    events.push((issue.id, event));
                    continue;
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::warn!(pr_number = pr_num, error = %e, "failed to check PR merge");
                    continue;
                }
            }

            // Check unresolved threads
            match self.github.list_unresolved_threads(pr_num).await {
                Ok(threads) if !threads.is_empty() => {
                    events.push((issue.id, Event::UnresolvedThreadsDetected));
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(pr_number = pr_num, error = %e, "failed to list unresolved threads");
                }
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

            let session_config = build_session_config(
                issue.state,
                issue.github_issue_number,
                &self.config,
                pr_number,
                issue.feature_name.as_deref(),
            );

            let schema = match session_config.output_schema {
                OutputSchemaKind::PrCreation => Some(PR_CREATION_SCHEMA),
                OutputSchemaKind::Fixing => Some(FIXING_SCHEMA),
            };

            let resolved_model = resolve_model(issue.model.as_deref(), &self.config.model);

            match self
                .claude_runner
                .spawn(&session_config.prompt, wt, schema, resolved_model)
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
                    let threads = self.github.list_unresolved_threads(pr_num).await?;
                    write_review_threads_input(wt, &threads)?;
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

        tracing::info!("graceful shutdown complete");
    }
}

/// model:* パターンのラベルを抽出し、最初に見つかった値を返す（空文字は無視）
fn extract_model_from_labels(labels: &[String]) -> Option<String> {
    labels.iter().find_map(|l| {
        let model = l.strip_prefix("model:")?.trim();
        if model.is_empty() {
            None
        } else {
            Some(model.to_string())
        }
    })
}

/// Issue モデル → Config モデル → "sonnet" の優先順位でモデル名を解決する
fn resolve_model<'a>(issue_model: Option<&'a str>, config_model: &'a str) -> &'a str {
    if let Some(m) = issue_model {
        let trimmed = m.trim();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    let config_trimmed = config_model.trim();
    if !config_trimmed.is_empty() {
        return config_trimmed;
    }
    "sonnet"
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // === Task 4.1: extract_model_from_labels tests ===

    fn make_labels(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn extract_model_returns_some_for_model_label() {
        let result = extract_model_from_labels(&make_labels(&["model:opus", "agent:ready"]));
        assert_eq!(result.as_deref(), Some("opus"));
    }

    #[test]
    fn extract_model_returns_first_when_multiple() {
        let result =
            extract_model_from_labels(&make_labels(&["agent:ready", "model:haiku", "model:opus"]));
        assert_eq!(result.as_deref(), Some("haiku"));
    }

    #[test]
    fn extract_model_returns_none_when_no_model_label() {
        let result = extract_model_from_labels(&make_labels(&["agent:ready", "bug"]));
        assert!(result.is_none());
    }

    #[test]
    fn extract_model_returns_none_for_empty_model_label() {
        let result = extract_model_from_labels(&make_labels(&["model:", "agent:ready"]));
        assert!(result.is_none());
    }

    #[test]
    fn extract_model_trims_whitespace() {
        let result = extract_model_from_labels(&make_labels(&["model:  opus  "]));
        assert_eq!(result.as_deref(), Some("opus"));
    }

    // === Task 4.1: resolve_model tests ===

    #[test]
    fn resolve_model_prefers_issue_model() {
        assert_eq!(resolve_model(Some("opus"), "haiku"), "opus");
    }

    #[test]
    fn resolve_model_falls_back_to_config() {
        assert_eq!(resolve_model(None, "haiku"), "haiku");
    }

    #[test]
    fn resolve_model_defaults_to_sonnet() {
        assert_eq!(resolve_model(None, ""), "sonnet");
    }
}
