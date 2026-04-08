/// Execute phase: run effects decided by Decide.
///
/// Effects are executed in priority order. Non-best-effort effect failures abort
/// the chain. Best-effort failures are logged and the chain continues.
use std::path::Path;

use anyhow::Result;

use crate::application::init_task_manager::InitTaskManager;
use crate::application::io::{clear_inputs_dir, write_issue_input, write_review_threads_input};
use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::git_worktree::GitWorktree;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::application::prompt::{
    FIXING_SCHEMA, OutputSchemaKind, PR_CREATION_SCHEMA, build_session_config,
};
use crate::application::session_manager::SessionManager;
use crate::domain::config::Config;
use crate::domain::effect::Effect;
use crate::domain::issue::Issue;
use crate::domain::metadata_update::MetadataUpdates;
use crate::domain::process_run::{ProcessRun, ProcessRunType};

/// Execute all effects for one issue.
///
/// Effects are processed in priority order (already sorted by `Decision::new`).
/// A non-best-effort failure aborts the rest. Best-effort failures are logged only.
#[allow(clippy::too_many_arguments)]
pub async fn execute_effects<G, I, P, C, W>(
    github: &G,
    issue_repo: &I,
    process_repo: &P,
    claude_runner: &C,
    worktree: &W,
    session_mgr: &mut SessionManager,
    init_mgr: &mut InitTaskManager,
    config: &Config,
    issue: &mut Issue,
    effects: &[Effect],
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: GitWorktree,
{
    for effect in effects {
        let best_effort = effect.is_best_effort();
        let result = execute_one(
            github,
            issue_repo,
            process_repo,
            claude_runner,
            worktree,
            session_mgr,
            init_mgr,
            config,
            issue,
            effect,
        )
        .await;

        match result {
            Ok(()) => {}
            Err(e) => {
                if best_effort {
                    tracing::warn!(
                        issue_number = issue.github_issue_number,
                        effect = ?effect,
                        error = %e,
                        "best-effort effect failed, continuing"
                    );
                } else {
                    tracing::error!(
                        issue_number = issue.github_issue_number,
                        effect = ?effect,
                        error = %e,
                        "effect failed, aborting remaining effects for this issue"
                    );
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn execute_one<G, I, P, C, W>(
    github: &G,
    issue_repo: &I,
    process_repo: &P,
    claude_runner: &C,
    worktree: &W,
    session_mgr: &mut SessionManager,
    init_mgr: &mut InitTaskManager,
    config: &Config,
    issue: &mut Issue,
    effect: &Effect,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: GitWorktree,
{
    let n = issue.github_issue_number;
    let lang = &config.language;

    match effect {
        Effect::PostCompletedComment => {
            let msg = rust_i18n::t!("issue_comment.all_completed", locale = lang);
            github.comment_on_issue(n, &msg).await?;
        }

        Effect::PostCancelComment => {
            let msg = rust_i18n::t!("issue_comment.cleanup_done", locale = lang);
            github.comment_on_issue(n, &msg).await?;
        }

        Effect::PostRetryExhaustedComment => {
            let unknown = rust_i18n::t!("issue_comment.unknown_error", locale = lang);
            // Find the last error message from the most recent failed process run
            let last_error = find_last_error(process_repo, issue.id).await;
            let error_str = last_error.as_deref().unwrap_or(&unknown);
            // Count consecutive failures across all process types
            let count = count_total_failures(process_repo, issue.id).await;
            let msg = rust_i18n::t!(
                "issue_comment.retry_exhausted",
                locale = lang,
                count = count,
                error = error_str
            );
            github.comment_on_issue(n, &msg).await?;
        }

        Effect::RejectUntrustedReadyIssue => {
            // Remove label first (if that fails, skip comment)
            match github.remove_label(n, "agent:ready").await {
                Ok(()) => {
                    // Post comment
                    let msg = "This issue was labeled `agent:ready` by an untrusted actor. \
                         Only trusted collaborators may trigger automatic processing."
                        .to_string();
                    let _ = github.comment_on_issue(n, &msg).await;
                }
                Err(e) => {
                    tracing::warn!(issue_number = n, error = %e, "failed to remove agent:ready label");
                }
            }
        }

        Effect::PostCiFixLimitComment => {
            let msg = format!(
                "CI fix limit reached ({} cycles). Automatic fixing has stopped.",
                config.max_ci_fix_cycles
            );
            github.comment_on_issue(n, &msg).await?;
        }

        Effect::SpawnInit => {
            spawn_init_task(
                github,
                issue_repo,
                process_repo,
                worktree,
                init_mgr,
                config,
                issue,
            )
            .await?;
        }

        Effect::SwitchToImplBranch => {
            let wt_path = issue
                .worktree_path
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("worktree_path is None for SwitchToImplBranch"))?;
            let path = Path::new(wt_path);
            let branch = format!("cupola/{}/main", issue.feature_name);
            worktree.checkout(path, &branch)?;
            worktree.pull(path)?;
        }

        Effect::SpawnProcess { type_, causes } => {
            spawn_process(
                github,
                issue_repo,
                process_repo,
                claude_runner,
                worktree,
                session_mgr,
                config,
                issue,
                *type_,
                causes,
            )
            .await?;
        }

        Effect::CleanupWorktree => {
            if let Some(ref wt) = issue.worktree_path {
                let path = Path::new(wt);
                if path.exists() {
                    worktree.remove(path)?;
                }

                // Clear worktree_path in DB
                let updates = MetadataUpdates {
                    worktree_path: Some(None),
                    ..Default::default()
                };
                issue_repo
                    .update_state_and_metadata(issue.id, &updates)
                    .await?;
                issue.worktree_path = None;
            }
        }

        Effect::CloseIssue => {
            github.close_issue(n).await?;

            // Mark close_finished = true
            let updates = MetadataUpdates {
                close_finished: Some(true),
                ..Default::default()
            };
            issue_repo
                .update_state_and_metadata(issue.id, &updates)
                .await?;
            issue.close_finished = true;
        }
    }

    Ok(())
}

async fn spawn_init_task<G, I, P, W>(
    github: &G,
    issue_repo: &I,
    process_repo: &P,
    worktree: &W,
    init_mgr: &mut InitTaskManager,
    config: &Config,
    issue: &Issue,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    W: GitWorktree,
{
    if init_mgr.is_active(issue.id) {
        tracing::debug!(issue_id = issue.id, "init task already active, skipping");
        return Ok(());
    }

    // Get next index
    let latest = process_repo
        .find_latest(issue.id, ProcessRunType::Init)
        .await?;
    let index = latest.map(|r| r.index + 1).unwrap_or(0);

    // Insert ProcessRun
    let run = ProcessRun::new_running(issue.id, ProcessRunType::Init, index, vec![]);
    let run_id = process_repo.save(&run).await?;

    // Fetch issue detail for spec generation
    let _detail = github.get_issue(issue.github_issue_number).await?;

    let feature_name = issue.feature_name.clone();
    let worktree_base = config
        .log_dir
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("worktrees");
    let wt_path = worktree_base.join(&feature_name);
    let default_branch = config.default_branch.clone();

    // We can't move the worktree trait into the async task (it may not be Send + 'static).
    // Instead, capture the paths and use std::process/std::fs directly in the task.
    // For now, we do the init synchronously and wrap it as a completed task.
    // TODO(phase 4): Make GitWorktree Send + 'static for true async init.
    let wt_path_clone = wt_path.clone();

    // Perform init synchronously (blocking the polling loop briefly)
    // This will be moved to a real async task in Phase 4.
    let result = perform_init_sync(worktree, &wt_path, &feature_name, &default_branch);

    match result {
        Ok(()) => {
            process_repo.mark_succeeded(run_id, None).await?;
            let worktree_path_str = wt_path_clone.to_string_lossy().to_string();
            let updates = MetadataUpdates {
                worktree_path: Some(Some(worktree_path_str)),
                ..Default::default()
            };
            issue_repo
                .update_state_and_metadata(issue.id, &updates)
                .await?;

            // Register a trivially-completed task to satisfy init_mgr tracking
            let handle =
                tokio::spawn(async move { Ok(wt_path_clone.to_string_lossy().to_string()) });
            init_mgr.register(issue.id, handle);
        }
        Err(e) => {
            process_repo
                .mark_failed(run_id, Some(e.to_string()))
                .await?;
        }
    }

    Ok(())
}

fn perform_init_sync<W: GitWorktree>(
    worktree: &W,
    wt_path: &Path,
    feature_name: &str,
    default_branch: &str,
) -> Result<()> {
    worktree.fetch()?;

    let main_branch = format!("cupola/{feature_name}/main");
    let start_point = format!("origin/{default_branch}");
    worktree.create(wt_path, &main_branch, &start_point)?;

    let design_branch = format!("cupola/{feature_name}/design");
    worktree.create_branch(wt_path, &design_branch)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn spawn_process<G, I, P, C, W>(
    github: &G,
    _issue_repo: &I,
    process_repo: &P,
    claude_runner: &C,
    worktree: &W,
    session_mgr: &mut SessionManager,
    config: &Config,
    issue: &Issue,
    type_: ProcessRunType,
    causes: &[crate::domain::fixing_problem_kind::FixingProblemKind],
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: GitWorktree,
{
    // Check session limit
    if let Some(max) = config.max_concurrent_sessions
        && session_mgr.count() >= max as usize
    {
        tracing::info!(
            issue_number = issue.github_issue_number,
            "concurrent session limit reached, deferring spawn"
        );
        return Ok(());
    }

    let wt_path_str = issue
        .worktree_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("worktree_path is None for SpawnProcess"))?;
    let wt_path = Path::new(wt_path_str);

    // Get next index for this type
    let latest = process_repo.find_latest(issue.id, type_).await?;
    let index = latest.map(|r| r.index + 1).unwrap_or(0);

    // Fetch issue detail for input files
    let detail = github.get_issue(issue.github_issue_number).await?;

    // Clear and write input files
    clear_inputs_dir(wt_path)?;
    write_issue_input(wt_path, &detail)?;

    // For fixing phases: fetch + merge, write review threads
    let pr_number_opt = get_pr_number_for_type(process_repo, issue.id, type_).await?;
    if matches!(type_, ProcessRunType::DesignFix | ProcessRunType::ImplFix) {
        // Merge latest default branch
        if let Err(e) = worktree.merge(wt_path, &format!("origin/{}", config.default_branch)) {
            tracing::warn!(error = %e, "merge failed before fixing spawn, proceeding anyway");
        }

        // Write review threads
        if let Some(pr_number) = pr_number_opt {
            let threads = github.list_unresolved_threads(pr_number).await?;
            let trusted = &config.trusted_associations;
            write_review_threads_input(wt_path, &threads, trusted)?;
        }
    }

    // Build session config
    let state = crate::domain::phase::Phase::from_state(issue.state)
        .map(|p| state_from_phase(p, type_))
        .unwrap_or(issue.state);

    let session_config = build_session_config(
        state,
        issue.github_issue_number,
        config,
        pr_number_opt,
        &issue.feature_name,
        causes,
        false, // has_merge_conflict: TODO detect from ProcessRun
    )?;

    let schema = match session_config.output_schema {
        OutputSchemaKind::PrCreation => Some(PR_CREATION_SCHEMA),
        OutputSchemaKind::Fixing => Some(FIXING_SCHEMA),
    };

    // Select model
    let model_phase = phase_for_type(type_);
    let model = config.models.resolve(issue.weight, model_phase);

    // Insert ProcessRun
    let run = ProcessRun::new_running(issue.id, type_, index, causes.to_vec());
    let run_id = process_repo.save(&run).await?;

    // Spawn the process
    match claude_runner.spawn(&session_config.prompt, wt_path, schema, model) {
        Ok(child) => {
            session_mgr.register(issue.id, issue.state, child);
            session_mgr.update_run_id(issue.id, run_id);
            tracing::info!(
                issue_number = issue.github_issue_number,
                run_id,
                type_ = ?type_,
                "spawned process"
            );
        }
        Err(e) => {
            // Spawn failure: mark ProcessRun as failed immediately
            let _ = process_repo.mark_failed(run_id, Some(e.to_string())).await;
            return Err(e);
        }
    }

    Ok(())
}

async fn get_pr_number_for_type<P: ProcessRunRepository>(
    process_repo: &P,
    issue_id: i64,
    type_: ProcessRunType,
) -> Result<Option<u64>> {
    // For fix types, look up the parent running type's PR
    let lookup_type = match type_ {
        ProcessRunType::DesignFix => ProcessRunType::Design,
        ProcessRunType::ImplFix => ProcessRunType::Impl,
        _ => type_,
    };
    let latest = process_repo.find_latest(issue_id, lookup_type).await?;
    Ok(latest.and_then(|r| r.pr_number))
}

fn state_from_phase(
    phase: crate::domain::phase::Phase,
    type_: ProcessRunType,
) -> crate::domain::state::State {
    use crate::domain::phase::Phase;
    use crate::domain::state::State;
    use ProcessRunType::*;

    match (phase, type_) {
        (Phase::Design, Design) => State::DesignRunning,
        (Phase::DesignFix, DesignFix) => State::DesignFixing,
        (Phase::Implementation, Impl) => State::ImplementationRunning,
        (Phase::ImplementationFix, ImplFix) => State::ImplementationFixing,
        _ => State::DesignRunning, // fallback
    }
}

fn phase_for_type(type_: ProcessRunType) -> Option<crate::domain::phase::Phase> {
    use crate::domain::phase::Phase;
    match type_ {
        ProcessRunType::Init => None,
        ProcessRunType::Design => Some(Phase::Design),
        ProcessRunType::DesignFix => Some(Phase::DesignFix),
        ProcessRunType::Impl => Some(Phase::Implementation),
        ProcessRunType::ImplFix => Some(Phase::ImplementationFix),
    }
}

async fn find_last_error<P: ProcessRunRepository>(
    process_repo: &P,
    issue_id: i64,
) -> Option<String> {
    let runs = process_repo.find_by_issue(issue_id).await.ok()?;
    runs.into_iter()
        .rev()
        .find(|r| r.state == crate::domain::process_run::ProcessRunState::Failed)
        .and_then(|r| r.error_message)
}

async fn count_total_failures<P: ProcessRunRepository>(process_repo: &P, issue_id: i64) -> u32 {
    let runs = process_repo
        .find_by_issue(issue_id)
        .await
        .unwrap_or_default();
    runs.into_iter()
        .filter(|r| r.state == crate::domain::process_run::ProcessRunState::Failed)
        .count() as u32
}

#[cfg(test)]
mod tests {
    use crate::domain::effect::Effect;
    use crate::domain::fixing_problem_kind::FixingProblemKind;
    use crate::domain::process_run::ProcessRunType;

    /// T-3.EX.1: effects are executed in priority order
    #[test]
    fn effect_priority_order() {
        let mut effects = [
            Effect::CloseIssue,
            Effect::PostCompletedComment,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
            },
        ];
        effects.sort_by_key(|e| e.priority());
        assert_eq!(effects[0], Effect::PostCompletedComment); // priority 2 (transition)
        assert!(matches!(effects[1], Effect::SpawnProcess { .. })); // priority 5
        assert_eq!(effects[2], Effect::CloseIssue); // priority 7
    }

    /// T-3.EX.2: best-effort effects are correctly identified
    #[test]
    fn best_effort_classification() {
        assert!(Effect::PostCompletedComment.is_best_effort());
        assert!(Effect::PostCancelComment.is_best_effort());
        assert!(Effect::PostRetryExhaustedComment.is_best_effort());
        assert!(Effect::RejectUntrustedReadyIssue.is_best_effort());
        assert!(Effect::PostCiFixLimitComment.is_best_effort());
        assert!(Effect::CleanupWorktree.is_best_effort());
        assert!(Effect::CloseIssue.is_best_effort());

        assert!(!Effect::SpawnInit.is_best_effort());
        assert!(!Effect::SwitchToImplBranch.is_best_effort());
        assert!(
            !Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![]
            }
            .is_best_effort()
        );
    }

    /// T-3.EX.3: FixingProblemKind causes are passed through
    #[test]
    fn fixing_causes_in_spawn_process() {
        let causes = vec![FixingProblemKind::CiFailure, FixingProblemKind::Conflict];
        let effect = Effect::SpawnProcess {
            type_: ProcessRunType::DesignFix,
            causes: causes.clone(),
        };
        if let Effect::SpawnProcess {
            type_: t,
            causes: c,
        } = effect
        {
            assert_eq!(t, ProcessRunType::DesignFix);
            assert_eq!(c, causes);
        } else {
            panic!("expected SpawnProcess");
        }
    }
}
