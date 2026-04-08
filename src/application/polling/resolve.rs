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
        let (head_branch, base_branch) = if session.registered_state == State::DesignRunning {
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
