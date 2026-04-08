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
                config.default_branch.clone(),
            )
        } else {
            // ImplementationRunning
            (
                format!("cupola/{feature}/main"),
                config.default_branch.clone(),
            )
        };

        // Idempotent: check if PR already exists on this branch
        let pr_number = match github
            .find_pr_by_branches(&head_branch, &base_branch)
            .await?
        {
            Some(pr) => {
                tracing::info!(
                    issue_number = n,
                    pr_number = pr.number,
                    "PR already exists on branch, reusing"
                );
                pr.number
            }
            None => {
                github
                    .create_pr(&head_branch, &base_branch, &title, &body)
                    .await?
            }
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
    use crate::application::session_manager::SessionManager;
    use std::process::{Command, Stdio};
    use std::time::Duration;

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
