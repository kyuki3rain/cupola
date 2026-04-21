/// Resolve phase: collect exited processes, update ProcessRun records, handle init tasks.
///
/// This phase collects the results of async operations from the previous cycle(s)
/// and updates the DB before the Collect phase observes the new state.
use anyhow::Result;

use crate::application::init_task_manager::InitTaskManager;
use crate::application::io::{parse_fixing_output, parse_pr_creation_output};
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::application::session_manager::{ExitedSession, SessionManager};
use crate::domain::config::Config;
use crate::domain::metadata_update::MetadataUpdates;
use crate::domain::state::State;

/// Max characters of stderr included in `error_message` for a failed process.
/// Longer stderr payloads are truncated; the full output remains in the log file.
const STDERR_SNIPPET_MAX: usize = 512;

/// Remove GitHub "closing keyword" references (e.g. `Closes #42`, `fixes owner/repo#5`)
/// from a PR body. Used for **design PRs only** — merging a design PR must not auto-close
/// the parent issue, because the issue lifecycle is owned by cupola (an impl PR merge
/// is the legitimate close trigger, and the CloseIssue effect handles that path).
///
/// Matches the case-insensitive verbs GitHub recognises — close/closes/closed,
/// fix/fixes/fixed, resolve/resolves/resolved — followed by an issue reference of
/// the form `#N` or `owner/repo#N`. Non-issue sentences like "this closes the gap"
/// are left alone because they lack a `#number`.
fn strip_closing_keywords(body: &str) -> String {
    const VERBS: &[&str] = &[
        "closes", "closed", "close", "fixes", "fixed", "fix", "resolves", "resolved", "resolve",
    ];

    let bytes = body.as_bytes();
    let lower: Vec<u8> = bytes.iter().map(u8::to_ascii_lowercase).collect();
    let mut out: Vec<u8> = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < bytes.len() {
        // Must start at a word boundary
        let at_boundary = i == 0 || !lower[i - 1].is_ascii_alphanumeric();
        let mut matched = None;
        if at_boundary {
            for verb in VERBS {
                let vb = verb.as_bytes();
                if i + vb.len() <= lower.len()
                    && lower[i..i + vb.len()] == *vb
                    && (i + vb.len() == lower.len() || !lower[i + vb.len()].is_ascii_alphanumeric())
                {
                    // Check for whitespace + optional owner/repo + # + digits
                    let mut j = i + vb.len();
                    let ws_start = j;
                    while j < lower.len() && (lower[j] == b' ' || lower[j] == b'\t') {
                        j += 1;
                    }
                    if j == ws_start {
                        continue;
                    }
                    // Optional owner/repo: [A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+
                    let or_start = j;
                    while j < lower.len()
                        && (lower[j].is_ascii_alphanumeric()
                            || matches!(lower[j], b'_' | b'.' | b'-'))
                    {
                        j += 1;
                    }
                    if j > or_start && j < lower.len() && lower[j] == b'/' {
                        j += 1;
                        let repo_start = j;
                        while j < lower.len()
                            && (lower[j].is_ascii_alphanumeric()
                                || matches!(lower[j], b'_' | b'.' | b'-'))
                        {
                            j += 1;
                        }
                        if j == repo_start {
                            continue;
                        }
                    } else {
                        j = or_start;
                    }
                    if j >= lower.len() || lower[j] != b'#' {
                        continue;
                    }
                    j += 1;
                    let num_start = j;
                    while j < lower.len() && lower[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j == num_start {
                        continue;
                    }
                    // Consume trailing punctuation + whitespace
                    if j < lower.len() && matches!(lower[j], b',' | b'.' | b';' | b':') {
                        j += 1;
                    }
                    while j < lower.len() && (lower[j] == b' ' || lower[j] == b'\t') {
                        j += 1;
                    }
                    matched = Some(j);
                    break;
                }
            }
        }
        if let Some(end) = matched {
            i = end;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    // SAFETY boundary: we only skip full ASCII byte runs (verbs + ASCII separators
    // + `#` + digits) and copy every other byte verbatim, so UTF-8 boundaries are
    // preserved.
    String::from_utf8(out).unwrap_or_else(|_| body.to_string())
}

/// Persist full stdout/stderr of a finished process to
/// `.cupola/logs/process-runs/run-{run_id}-{stdout|stderr}.log` (best-effort).
/// This is critical for debugging Claude failures where stderr may be empty
/// but stdout contains the real diagnostic (e.g. Claude JSON error payload).
fn dump_session_io(run_id: i64, stdout: &str, stderr: &str) {
    if run_id <= 0 {
        return;
    }
    let dir = std::path::Path::new(".cupola/logs/process-runs");
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "failed to create process-runs log dir");
        return;
    }
    for (name, content) in [("stdout", stdout), ("stderr", stderr)] {
        let path = dir.join(format!("run-{run_id}-{name}.log"));
        if let Err(e) = std::fs::write(&path, content) {
            tracing::warn!(path = %path.display(), error = %e, "failed to write process log");
        }
    }
}

/// Process all exited sessions and update ProcessRun + Issue metadata.
pub async fn resolve_exited_sessions<G, I, P>(
    github: &G,
    issue_repo: &I,
    process_repo: &P,
    config: &Config,
    session_mgr: &mut SessionManager,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
{
    let exited = session_mgr.collect_exited();

    for session in exited {
        if let Err(e) =
            process_exited_session(github, issue_repo, process_repo, config, &session).await
        {
            tracing::warn!(
                issue_id = session.issue_id,
                error = %e,
                "resolve failed for session, continuing"
            );
        }
    }

    Ok(())
}

async fn process_exited_session<G, I, P>(
    github: &G,
    issue_repo: &I,
    process_repo: &P,
    config: &Config,
    session: &ExitedSession,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
{
    let issue_id = session.issue_id;
    let run_id = session.run_id;

    // Stale guard: if the issue's current state differs from the registered state,
    // mark as stale instead of doing post-processing.
    let current_issue = match issue_repo.find_by_id(issue_id).await? {
        Some(i) => i,
        None => {
            tracing::warn!(issue_id, "issue not found in DB during resolve, skipping");
            return Ok(());
        }
    };

    if session.registered_state != current_issue.state {
        tracing::info!(
            issue_id,
            registered_state = ?session.registered_state,
            current_state = ?current_issue.state,
            "stale guard: registered state differs from current, marking run as stale"
        );
        if run_id > 0 {
            // mark_stale sets state=stale, pid=NULL, finished_at=now() so the row
            // is properly terminated and stops counting toward consecutive_failures.
            process_repo.mark_stale(run_id).await?;
        }
        return Ok(());
    }

    // Always dump stdout+stderr to .cupola/logs/process-runs/ before any
    // success/failure branching. Essential for diagnosing Claude exit code 1
    // with empty stderr, JSON schema mismatches, etc.
    dump_session_io(run_id, &session.stdout, &session.stderr);

    let is_running_phase = matches!(
        session.registered_state,
        State::DesignRunning | State::ImplementationRunning
    );
    let is_fixing_phase = matches!(
        session.registered_state,
        State::DesignFixing | State::ImplementationFixing
    );

    if !session.exit_status.success() {
        // Process failed: capture exit code + stderr snippet for diagnostics.
        let exit_repr = match session.exit_status.code() {
            Some(code) => format!("exit code {code}"),
            None => "terminated by signal".to_string(),
        };
        let stderr_trimmed = session.stderr.trim();
        let stderr_snippet = if stderr_trimmed.is_empty() {
            String::new()
        } else if stderr_trimmed.len() <= STDERR_SNIPPET_MAX {
            format!(": {stderr_trimmed}")
        } else {
            let head: String = stderr_trimmed.chars().take(STDERR_SNIPPET_MAX).collect();
            format!(": {head}… (truncated)")
        };
        let error_message = format!("{exit_repr}{stderr_snippet}");

        if run_id > 0 {
            process_repo
                .mark_failed(run_id, Some(error_message))
                .await?;
        }
        return Ok(());
    }

    // Process succeeded
    if is_running_phase {
        // Running phase: parse PR creation output, create PR on GitHub
        let output = parse_pr_creation_output(&session.stdout);
        let (title, body) = match output {
            Some(o) => (
                o.pr_title.unwrap_or_else(|| {
                    format!("PR for issue #{}", current_issue.github_issue_number)
                }),
                o.pr_body.unwrap_or_default(),
            ),
            None => (
                format!("PR for issue #{}", current_issue.github_issue_number),
                String::new(),
            ),
        };

        let n = current_issue.github_issue_number;
        let feature = &current_issue.feature_name;
        let is_design = session.registered_state == State::DesignRunning;
        // Design PRs must not carry "Closes #N" — the issue belongs to cupola's
        // state machine; only an impl PR merge should close it.
        let body = if is_design {
            strip_closing_keywords(&body)
        } else {
            body
        };
        let (head_branch, base_branch) = if is_design {
            (
                format!("cupola/{feature}/design"),
                format!("cupola/{feature}/main"),
            )
        } else {
            // ImplementationRunning
            (
                format!("cupola/{feature}/main"),
                config.default_branch.clone(),
            )
        };

        // Idempotent: check if PR already exists on this branch
        let find_result = github.find_pr_by_branches(&head_branch, &base_branch).await;

        let pr_number = match find_result {
            Err(e) => {
                tracing::warn!(
                    run_id,
                    head_branch = %head_branch,
                    base_branch = %base_branch,
                    error = %e,
                    "GitHub API error in Running phase (find_pr_by_branches)"
                );
                if run_id > 0 {
                    process_repo
                        .mark_failed(run_id, Some(e.to_string()))
                        .await?;
                }
                return Ok(());
            }
            Ok(Some(pr)) => {
                tracing::info!(
                    issue_number = n,
                    pr_number = pr.number,
                    "PR already exists on branch, reusing"
                );
                pr.number
            }
            Ok(None) => match github
                .create_pr(&head_branch, &base_branch, &title, &body)
                .await
            {
                Ok(pr_num) => pr_num,
                Err(e) => {
                    tracing::warn!(
                        run_id,
                        head_branch = %head_branch,
                        base_branch = %base_branch,
                        error = %e,
                        "GitHub API error in Running phase (create_pr)"
                    );
                    if run_id > 0 {
                        process_repo
                            .mark_failed(run_id, Some(e.to_string()))
                            .await?;
                    }
                    return Ok(());
                }
            },
        };

        if run_id > 0 {
            process_repo.mark_succeeded(run_id, Some(pr_number)).await?;
        }
    } else if is_fixing_phase {
        // Fixing phase: parse fixing output, reply to threads
        let output = parse_fixing_output(&session.stdout);
        if let Some(fixing) = output {
            for thread in &fixing.threads {
                if let Err(e) = github
                    .reply_to_thread(&thread.thread_id, &thread.response)
                    .await
                {
                    tracing::warn!(error = %e, thread_id = %thread.thread_id, "failed to reply to thread");
                }
                if thread.resolved
                    && let Err(e) = github.resolve_thread(&thread.thread_id).await
                {
                    tracing::warn!(error = %e, thread_id = %thread.thread_id, "failed to resolve thread");
                }
            }
        }

        if run_id > 0 {
            process_repo.mark_succeeded(run_id, None).await?;
        }
    } else {
        // Other states (e.g. InitializeRunning handled by init task manager)
        if run_id > 0 {
            process_repo.mark_succeeded(run_id, None).await?;
        }
    }

    Ok(())
}

/// Process completed init tasks from InitTaskManager.
pub async fn resolve_init_tasks<I, P>(
    issue_repo: &I,
    process_repo: &P,
    init_mgr: &mut InitTaskManager,
) -> Result<()>
where
    I: IssueRepository,
    P: ProcessRunRepository,
{
    let results = init_mgr.collect_finished().await;

    for result in results {
        let issue_id = result.issue_id;

        // Find the latest init ProcessRun for this issue
        let run = process_repo
            .find_latest(issue_id, crate::domain::process_run::ProcessRunType::Init)
            .await?;

        match result.outcome {
            Ok(worktree_path) => {
                tracing::info!(
                    issue_id,
                    worktree_path = %worktree_path,
                    "init task completed successfully"
                );

                // Update ProcessRun to succeeded
                if let Some(run) = run {
                    process_repo.mark_succeeded(run.id, None).await?;
                }

                // Update Issue.worktree_path
                let updates = MetadataUpdates {
                    worktree_path: Some(Some(worktree_path)),
                    ..Default::default()
                };
                issue_repo
                    .update_state_and_metadata(issue_id, &updates)
                    .await?;
            }
            Err(e) => {
                tracing::warn!(
                    issue_id,
                    error = %e,
                    "init task failed"
                );

                // Update ProcessRun to failed
                if let Some(run) = run {
                    process_repo
                        .mark_failed(run.id, Some(e.to_string()))
                        .await?;
                }
            }
        }
    }

    Ok(())
}

/// Kill stalled processes.
pub fn kill_stalled(session_mgr: &mut SessionManager, stall_timeout_secs: u64) {
    let timeout = std::time::Duration::from_secs(stall_timeout_secs);
    let stalled = session_mgr.find_stalled(timeout);
    for issue_id in stalled {
        tracing::warn!(issue_id, "killing stalled process");
        let _ = session_mgr.kill(issue_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::github_client::{
        GitHubClient, GitHubIssueDetail, GitHubPr, GitHubPrDetails, RepositoryPermission,
        ReviewThread,
    };
    use crate::application::port::issue_repository::IssueRepository;
    use crate::application::port::process_run_repository::ProcessRunRepository;
    use crate::application::session_manager::{ExitedSession, SessionManager};
    use crate::domain::issue::Issue;
    use crate::domain::metadata_update::MetadataUpdates;
    use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};
    use crate::domain::task_weight::TaskWeight;
    use anyhow::Result;
    use chrono::{DateTime, Utc};
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    // ── Minimal mock: IssueRepository ────────────────────────────────────────

    struct MockIssueRepo {
        issue: Issue,
    }

    impl IssueRepository for MockIssueRepo {
        async fn find_by_id(&self, _id: i64) -> Result<Option<Issue>> {
            Ok(Some(self.issue.clone()))
        }
        async fn find_by_issue_number(&self, _n: u64) -> Result<Option<Issue>> {
            Ok(None)
        }
        async fn find_active(&self) -> Result<Vec<Issue>> {
            Ok(vec![])
        }
        async fn find_all(&self) -> Result<Vec<Issue>> {
            Ok(vec![])
        }
        async fn save(&self, _issue: &Issue) -> Result<i64> {
            Ok(0)
        }
        async fn update_state(&self, _id: i64, _state: State) -> Result<()> {
            Ok(())
        }
        async fn update(&self, _issue: &Issue) -> Result<()> {
            Ok(())
        }
        async fn update_state_and_metadata(
            &self,
            _id: i64,
            _updates: &MetadataUpdates,
        ) -> Result<()> {
            Ok(())
        }
        async fn find_by_state(&self, _state: State) -> Result<Vec<Issue>> {
            Ok(vec![])
        }
    }

    // ── Minimal mock: ProcessRunRepository ───────────────────────────────────

    #[derive(Default)]
    struct CallLog {
        mark_failed: Vec<(i64, Option<String>)>,
        mark_succeeded: Vec<(i64, Option<u64>)>,
    }

    struct MockProcessRepo {
        calls: Arc<Mutex<CallLog>>,
    }

    impl MockProcessRepo {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(CallLog::default())),
            }
        }
    }

    impl ProcessRunRepository for MockProcessRepo {
        async fn save(&self, _run: &ProcessRun) -> Result<i64> {
            Ok(0)
        }
        async fn update_pid(&self, _id: i64, _pid: u32) -> Result<()> {
            Ok(())
        }
        async fn mark_succeeded(&self, run_id: i64, pr_number: Option<u64>) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .mark_succeeded
                .push((run_id, pr_number));
            Ok(())
        }
        async fn mark_failed(&self, run_id: i64, error_message: Option<String>) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .mark_failed
                .push((run_id, error_message));
            Ok(())
        }
        async fn mark_stale(&self, _id: i64) -> Result<()> {
            Ok(())
        }
        async fn mark_stale_for_issue(&self, _id: i64) -> Result<()> {
            Ok(())
        }
        async fn find_latest(&self, _id: i64, _type: ProcessRunType) -> Result<Option<ProcessRun>> {
            Ok(None)
        }
        async fn find_latest_with_pr_number(
            &self,
            _id: i64,
            _type: ProcessRunType,
        ) -> Result<Option<ProcessRun>> {
            Ok(None)
        }
        async fn find_by_issue(&self, _id: i64) -> Result<Vec<ProcessRun>> {
            Ok(vec![])
        }
        async fn count_consecutive_failures(
            &self,
            _id: i64,
            _type: ProcessRunType,
            _since: Option<DateTime<Utc>>,
        ) -> Result<u32> {
            Ok(0)
        }
        async fn find_latest_with_consecutive_count(
            &self,
            _id: i64,
            _type: ProcessRunType,
            _since: Option<DateTime<Utc>>,
        ) -> Result<Option<(ProcessRun, u32)>> {
            Ok(None)
        }
        async fn find_all_running(&self) -> Result<Vec<ProcessRun>> {
            Ok(vec![])
        }
        async fn update_state(&self, _id: i64, _state: ProcessRunState) -> Result<()> {
            Ok(())
        }
    }

    // ── GitHub client mocks ───────────────────────────────────────────────────

    /// Always returns Err from find_pr_by_branches.
    struct GhFindPrError;

    impl GitHubClient for GhFindPrError {
        async fn list_open_issues(
            &self,
        ) -> Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }
        async fn get_issue(&self, _n: u64) -> Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number: 1,
                title: String::new(),
                body: String::new(),
                labels: vec![],
            })
        }
        async fn find_pr_by_branches(&self, _h: &str, _b: &str) -> Result<Option<GitHubPr>> {
            Err(anyhow::anyhow!("GitHub API unavailable"))
        }
        async fn is_pr_merged(&self, _n: u64) -> Result<bool> {
            Ok(false)
        }
        async fn list_unresolved_threads(&self, _n: u64) -> Result<Vec<ReviewThread>> {
            Ok(vec![])
        }
        async fn create_pr(&self, _h: &str, _b: &str, _t: &str, _body: &str) -> Result<u64> {
            Ok(1)
        }
        async fn reply_to_thread(&self, _id: &str, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn resolve_thread(&self, _id: &str) -> Result<()> {
            Ok(())
        }
        async fn comment_on_issue(&self, _n: u64, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn close_issue(&self, _n: u64) -> Result<()> {
            Ok(())
        }
        async fn get_job_logs(&self, _id: u64) -> Result<String> {
            Ok(String::new())
        }
        async fn get_pr_details(&self, _n: u64) -> Result<GitHubPrDetails> {
            Ok(GitHubPrDetails {
                merged: false,
                mergeable: Some(true),
            })
        }
        async fn fetch_label_actor_login(&self, _n: u64, _label: &str) -> Result<Option<String>> {
            Ok(None)
        }
        async fn fetch_user_permission(&self, _username: &str) -> Result<RepositoryPermission> {
            Ok(RepositoryPermission::Read)
        }
        async fn remove_label(&self, _n: u64, _label: &str) -> Result<()> {
            Ok(())
        }
        async fn observe_pr(
            &self,
            _n: u64,
        ) -> Result<Option<crate::application::port::github_client::PrObservation>> {
            Ok(None)
        }
    }

    /// Returns Ok(None) from find_pr_by_branches; always fails create_pr.
    struct GhCreatePrError;

    impl GitHubClient for GhCreatePrError {
        async fn list_open_issues(
            &self,
        ) -> Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }
        async fn get_issue(&self, _n: u64) -> Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number: 1,
                title: String::new(),
                body: String::new(),
                labels: vec![],
            })
        }
        async fn find_pr_by_branches(&self, _h: &str, _b: &str) -> Result<Option<GitHubPr>> {
            Ok(None)
        }
        async fn is_pr_merged(&self, _n: u64) -> Result<bool> {
            Ok(false)
        }
        async fn list_unresolved_threads(&self, _n: u64) -> Result<Vec<ReviewThread>> {
            Ok(vec![])
        }
        async fn create_pr(&self, _h: &str, _b: &str, _t: &str, _body: &str) -> Result<u64> {
            Err(anyhow::anyhow!("rate limit exceeded"))
        }
        async fn reply_to_thread(&self, _id: &str, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn resolve_thread(&self, _id: &str) -> Result<()> {
            Ok(())
        }
        async fn comment_on_issue(&self, _n: u64, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn close_issue(&self, _n: u64) -> Result<()> {
            Ok(())
        }
        async fn get_job_logs(&self, _id: u64) -> Result<String> {
            Ok(String::new())
        }
        async fn get_pr_details(&self, _n: u64) -> Result<GitHubPrDetails> {
            Ok(GitHubPrDetails {
                merged: false,
                mergeable: Some(true),
            })
        }
        async fn fetch_label_actor_login(&self, _n: u64, _label: &str) -> Result<Option<String>> {
            Ok(None)
        }
        async fn fetch_user_permission(&self, _username: &str) -> Result<RepositoryPermission> {
            Ok(RepositoryPermission::Read)
        }
        async fn remove_label(&self, _n: u64, _label: &str) -> Result<()> {
            Ok(())
        }
        async fn observe_pr(
            &self,
            _n: u64,
        ) -> Result<Option<crate::application::port::github_client::PrObservation>> {
            Ok(None)
        }
    }

    // ── GitHub client mock: records branch arguments ───────────────────────

    #[derive(Default)]
    struct BranchCallLog {
        find_pr_calls: Vec<(String, String)>, // (head, base)
    }

    struct GhBranchRecorder {
        calls: Arc<Mutex<BranchCallLog>>,
    }

    impl GhBranchRecorder {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(BranchCallLog::default())),
            }
        }
    }

    impl GitHubClient for GhBranchRecorder {
        async fn list_open_issues(
            &self,
        ) -> Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }
        async fn get_issue(&self, _n: u64) -> Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number: 1,
                title: String::new(),
                body: String::new(),
                labels: vec![],
            })
        }
        async fn find_pr_by_branches(&self, h: &str, b: &str) -> Result<Option<GitHubPr>> {
            self.calls
                .lock()
                .unwrap()
                .find_pr_calls
                .push((h.to_string(), b.to_string()));
            // Return existing PR to avoid create_pr call
            Ok(Some(GitHubPr {
                number: 99,
                merged: false,
            }))
        }
        async fn is_pr_merged(&self, _n: u64) -> Result<bool> {
            Ok(false)
        }
        async fn list_unresolved_threads(&self, _n: u64) -> Result<Vec<ReviewThread>> {
            Ok(vec![])
        }
        async fn create_pr(&self, _h: &str, _b: &str, _t: &str, _body: &str) -> Result<u64> {
            Ok(1)
        }
        async fn reply_to_thread(&self, _id: &str, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn resolve_thread(&self, _id: &str) -> Result<()> {
            Ok(())
        }
        async fn comment_on_issue(&self, _n: u64, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn close_issue(&self, _n: u64) -> Result<()> {
            Ok(())
        }
        async fn get_job_logs(&self, _id: u64) -> Result<String> {
            Ok(String::new())
        }
        async fn get_pr_details(&self, _n: u64) -> Result<GitHubPrDetails> {
            Ok(GitHubPrDetails {
                merged: false,
                mergeable: Some(true),
            })
        }
        async fn fetch_label_actor_login(&self, _n: u64, _label: &str) -> Result<Option<String>> {
            Ok(None)
        }
        async fn fetch_user_permission(&self, _username: &str) -> Result<RepositoryPermission> {
            Ok(RepositoryPermission::Read)
        }
        async fn remove_label(&self, _n: u64, _label: &str) -> Result<()> {
            Ok(())
        }
        async fn observe_pr(
            &self,
            _n: u64,
        ) -> Result<Option<crate::application::port::github_client::PrObservation>> {
            Ok(None)
        }
    }

    // ── Test helpers ─────────────────────────────────────────────────────────

    fn success_exit_status() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }

    fn make_session(run_id: i64, state: State) -> ExitedSession {
        ExitedSession {
            issue_id: 1,
            exit_status: success_exit_status(),
            stdout: String::new(),
            stderr: String::new(),
            log_id: 0,
            run_id,
            registered_state: state,
        }
    }

    fn make_issue(state: State) -> Issue {
        let now = Utc::now();
        Issue {
            id: 1,
            github_issue_number: 42,
            state,
            feature_name: "test-feature".to_string(),
            weight: TaskWeight::Medium,
            worktree_path: None,
            ci_fix_count: 0,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
            body_hash: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn unit_config() -> Config {
        Config::default_with_repo("owner".into(), "repo".into(), "main".into())
    }

    // ── Unit tests: task 2.1 ─────────────────────────────────────────────────

    /// T-2.1: find_pr_by_branches error → mark_failed called, Ok(()) returned, mark_succeeded not called
    #[tokio::test]
    async fn find_pr_error_calls_mark_failed_and_returns_ok() {
        let github = GhFindPrError;
        let issue_repo = MockIssueRepo {
            issue: make_issue(State::DesignRunning),
        };
        let process_repo = MockProcessRepo::new();
        let calls = process_repo.calls.clone();
        let config = unit_config();
        let session = make_session(42, State::DesignRunning);

        let result =
            process_exited_session(&github, &issue_repo, &process_repo, &config, &session).await;

        assert!(result.is_ok(), "expected Ok(()), got {result:?}");
        let log = calls.lock().unwrap();
        assert_eq!(
            log.mark_failed.len(),
            1,
            "mark_failed should be called once"
        );
        assert_eq!(log.mark_failed[0].0, 42, "run_id should match");
        assert!(
            log.mark_failed[0]
                .1
                .as_deref()
                .unwrap_or("")
                .contains("GitHub API unavailable"),
            "error_message should contain the original error"
        );
        assert!(
            log.mark_succeeded.is_empty(),
            "mark_succeeded must not be called on error"
        );
    }

    // ── Unit tests: task 2.2 ─────────────────────────────────────────────────

    /// T-2.2: create_pr error → mark_failed called, Ok(()) returned, mark_succeeded not called
    #[tokio::test]
    async fn create_pr_error_calls_mark_failed_and_returns_ok() {
        let github = GhCreatePrError;
        let issue_repo = MockIssueRepo {
            issue: make_issue(State::ImplementationRunning),
        };
        let process_repo = MockProcessRepo::new();
        let calls = process_repo.calls.clone();
        let config = unit_config();
        let session = make_session(7, State::ImplementationRunning);

        let result =
            process_exited_session(&github, &issue_repo, &process_repo, &config, &session).await;

        assert!(result.is_ok(), "expected Ok(()), got {result:?}");
        let log = calls.lock().unwrap();
        assert_eq!(
            log.mark_failed.len(),
            1,
            "mark_failed should be called once"
        );
        assert_eq!(log.mark_failed[0].0, 7, "run_id should match");
        assert!(
            log.mark_failed[0]
                .1
                .as_deref()
                .unwrap_or("")
                .contains("rate limit exceeded"),
            "error_message should contain the original error"
        );
        assert!(
            log.mark_succeeded.is_empty(),
            "mark_succeeded must not be called on error"
        );
    }

    // ── Unit tests: task 2.3 ─────────────────────────────────────────────────

    /// T-2.3: run_id == 0 with GitHub API error → mark_failed NOT called, Ok(()) returned
    #[tokio::test]
    async fn github_error_with_run_id_zero_skips_mark_failed() {
        let github = GhFindPrError;
        let issue_repo = MockIssueRepo {
            issue: make_issue(State::DesignRunning),
        };
        let process_repo = MockProcessRepo::new();
        let calls = process_repo.calls.clone();
        let config = unit_config();
        let session = make_session(0, State::DesignRunning);

        let result =
            process_exited_session(&github, &issue_repo, &process_repo, &config, &session).await;

        assert!(result.is_ok(), "expected Ok(()), got {result:?}");
        let log = calls.lock().unwrap();
        assert!(
            log.mark_failed.is_empty(),
            "mark_failed must not be called when run_id == 0"
        );
        assert!(
            log.mark_succeeded.is_empty(),
            "mark_succeeded must not be called on error"
        );
    }

    /// T-3.RS.1: kill_stalled kills a long-running process
    #[test]
    fn kill_stalled_kills_long_running() {
        let mut mgr = SessionManager::new();
        let child = Command::new("sleep")
            .arg("60")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");

        mgr.register(1, State::DesignRunning, child);

        kill_stalled(&mut mgr, 0); // 0 secs timeout → everything is stalled

        // Wait for kill to propagate
        std::thread::sleep(Duration::from_millis(100));

        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1, "stalled process should have been killed");
        assert!(!exited[0].exit_status.success());
    }

    #[test]
    fn strip_closing_keywords_removes_plain_ref() {
        assert_eq!(strip_closing_keywords("Closes #42"), "");
        assert_eq!(strip_closing_keywords("fixes #5 "), "");
        assert_eq!(strip_closing_keywords("Resolved #7."), "");
    }

    #[test]
    fn strip_closing_keywords_removes_cross_repo_ref() {
        assert_eq!(strip_closing_keywords("Fixes owner/repo#5"), "");
    }

    #[test]
    fn strip_closing_keywords_respects_word_boundary() {
        assert_eq!(strip_closing_keywords("fix a bug"), "fix a bug");
        assert_eq!(
            strip_closing_keywords("prefixes #1 should match"),
            "prefixes #1 should match"
        );
    }

    #[test]
    fn strip_closing_keywords_leaves_non_issue_sentences() {
        let s = "This PR closes the design issue.";
        assert_eq!(strip_closing_keywords(s), s);
    }

    #[test]
    fn strip_closing_keywords_preserves_surrounding_text() {
        assert_eq!(
            strip_closing_keywords("Overview.\n\nCloses #12\n\nDetails"),
            "Overview.\n\n\n\nDetails"
        );
    }

    #[test]
    fn strip_closing_keywords_preserves_non_ascii() {
        assert_eq!(
            strip_closing_keywords("設計PR. Closes #42 完了"),
            "設計PR. 完了"
        );
    }

    // ── Unit tests: base_branch ────────────────────────────────────────────

    /// T-2.base.1: Design PR の base は cupola/{feature}/main（impl branch）であること
    #[tokio::test]
    async fn design_pr_base_branch_is_impl_branch() {
        let github = GhBranchRecorder::new();
        let calls = github.calls.clone();
        let issue_repo = MockIssueRepo {
            issue: make_issue(State::DesignRunning),
        };
        let process_repo = MockProcessRepo::new();
        let config = unit_config();
        let session = make_session(1, State::DesignRunning);

        let _ =
            process_exited_session(&github, &issue_repo, &process_repo, &config, &session).await;

        let log = calls.lock().unwrap();
        assert_eq!(log.find_pr_calls.len(), 1, "find_pr should be called once");
        let (head, base) = &log.find_pr_calls[0];
        assert_eq!(head, "cupola/test-feature/design");
        assert_eq!(
            base, "cupola/test-feature/main",
            "design PR base should be impl branch, not default_branch"
        );
    }

    /// T-2.base.2: Impl PR の base は config.default_branch（main）であること（回帰）
    #[tokio::test]
    async fn impl_pr_base_branch_is_default_branch() {
        let github = GhBranchRecorder::new();
        let calls = github.calls.clone();
        let issue_repo = MockIssueRepo {
            issue: make_issue(State::ImplementationRunning),
        };
        let process_repo = MockProcessRepo::new();
        let config = unit_config();
        let session = make_session(1, State::ImplementationRunning);

        let _ =
            process_exited_session(&github, &issue_repo, &process_repo, &config, &session).await;

        let log = calls.lock().unwrap();
        assert_eq!(log.find_pr_calls.len(), 1, "find_pr should be called once");
        let (head, base) = &log.find_pr_calls[0];
        assert_eq!(head, "cupola/test-feature/main");
        assert_eq!(base, "main", "impl PR base should be default_branch");
    }

    // ── Log verification tests: task 3.2 ─────────────────────────────────────

    /// Minimal raw subscriber that records every WARN+ event's fields as a
    /// flat string.  Does not depend on `tracing_subscriber` internals, so it
    /// has no shared global state and is safe to use in parallel tests.
    struct CapturingSubscriber {
        events: Arc<Mutex<Vec<String>>>,
        next_id: std::sync::atomic::AtomicU64,
    }

    impl CapturingSubscriber {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    events: events.clone(),
                    next_id: std::sync::atomic::AtomicU64::new(1),
                },
                events,
            )
        }
    }

    impl tracing::Subscriber for CapturingSubscriber {
        fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
            metadata.level() <= &tracing::Level::WARN
        }
        fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            // Span IDs must be unique and non-zero.
            let id = self
                .next_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            tracing::span::Id::from_u64(id)
        }
        fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            struct Collector(String);
            impl tracing::field::Visit for Collector {
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    self.0.push_str(&format!(" {}={:?}", field.name(), value));
                }
                fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                    self.0.push_str(&format!(" {}={}", field.name(), value));
                }
                fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
                    self.0.push_str(&format!(" {}={}", field.name(), value));
                }
                fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
                    self.0.push_str(&format!(" {}={}", field.name(), value));
                }
            }
            let mut c = Collector(String::new());
            event.record(&mut c);
            self.events.lock().unwrap().push(c.0);
        }
        fn enter(&self, _span: &tracing::span::Id) {}
        fn exit(&self, _span: &tracing::span::Id) {}
    }

    /// T-3.2: GitHub API error emits tracing::warn! with run_id, head_branch,
    ///         base_branch, and error detail.
    ///
    /// Uses a minimal raw `tracing::Subscriber` (no global state) inside
    /// `with_default` + a local current-thread runtime for a fully isolated test.
    ///
    /// NOTE: Tracing callsite interests are cached globally. Parallel tests that
    /// run without a subscriber can cache Interest::Never for the warn! callsites,
    /// making this test flaky when run concurrently. Run with `--test-threads=1`
    /// or `cargo test github_error_emits_warn -- --ignored` to avoid the race.
    #[test]
    #[ignore = "flaky in parallel due to tracing callsite interest caching; run with --test-threads=1"]
    fn github_error_emits_warn_log_with_expected_fields() {
        let (sub, events) = CapturingSubscriber::new();

        tracing::subscriber::with_default(sub, || {
            // Rebuild callsite interest so that callsites already registered by
            // parallel tests (with no subscriber → Interest::Never) are updated
            // to reflect our thread-local subscriber.
            tracing_core::callsite::rebuild_interest_cache();

            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("build tokio rt");
            rt.block_on(async {
                let github = GhFindPrError;
                let issue_repo = MockIssueRepo {
                    issue: make_issue(State::DesignRunning),
                };
                let process_repo = MockProcessRepo::new();
                let config = unit_config();
                let session = make_session(99, State::DesignRunning);
                let _ =
                    process_exited_session(&github, &issue_repo, &process_repo, &config, &session)
                        .await;
            });
        });

        let captured = events.lock().unwrap().join(" ");
        assert!(
            captured.contains("99"),
            "log should contain run_id (99); captured: {captured:?}"
        );
        assert!(
            captured.contains("cupola/test-feature/design"),
            "log should contain head_branch; captured: {captured:?}"
        );
        assert!(
            captured.contains("main"),
            "log should contain base_branch; captured: {captured:?}"
        );
        assert!(
            captured.contains("GitHub API unavailable"),
            "log should contain error detail; captured: {captured:?}"
        );
    }

    /// T-3.RS.2: kill_stalled does not kill fresh process
    #[test]
    fn kill_stalled_skips_fresh_process() {
        let mut mgr = SessionManager::new();
        let child = Command::new("sleep")
            .arg("60")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");

        mgr.register(1, State::DesignRunning, child);

        kill_stalled(&mut mgr, 3600); // 1hr timeout → nothing stalled

        let exited = mgr.collect_exited();
        assert!(exited.is_empty(), "fresh process should not be killed");

        mgr.kill_all();
    }
}
