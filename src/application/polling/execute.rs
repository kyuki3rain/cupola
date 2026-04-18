/// Execute phase: run effects decided by Decide.
///
/// Effects are executed in priority order. Non-best-effort effect failures abort
/// the chain. Best-effort failures are logged and the chain continues.
use std::path::Path;
use std::process::Child;
use std::time::Duration;

use anyhow::Result;

use crate::application::init_task_manager::InitTaskManager;
use crate::application::io::{clear_inputs_dir, write_issue_input, write_review_threads_input};
use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::file_generator::FileGenerator;
use crate::application::port::git_worktree::{GitWorktree, MergeConflictError};
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
use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};

/// Backoff delays for DB update retries: [1s, 2s, 4s] — 3 retries after the
/// initial attempt (4 total calls maximum).
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

/// Retry a DB update closure with exponential backoff on transient failures.
///
/// Calls `db_update()` up to `RETRY_DELAYS.len() + 1` times. On each failure
/// (except the last), logs a `warn!` and sleeps the corresponding backoff
/// duration. After all retries are exhausted, logs an `error!` and returns
/// the last error.
async fn retry_db_update<F, Fut>(
    issue_number: u64,
    effect_name: &str,
    db_update: F,
) -> anyhow::Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    for (attempt, &delay) in RETRY_DELAYS.iter().enumerate() {
        match db_update().await {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing::warn!(
                    issue_number,
                    effect = effect_name,
                    attempt = attempt + 1,
                    error = %e,
                    "DB update failed, retrying after backoff"
                );
                tokio::time::sleep(delay).await;
            }
        }
    }
    // Final attempt after last backoff
    match db_update().await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!(
                issue_number,
                effect = effect_name,
                max_attempts = RETRY_DELAYS.len() + 1,
                error = %e,
                "DB update permanently failed after all retries"
            );
            Err(e)
        }
    }
}

/// Bound used anywhere SpawnInit needs to hand the worktree to a `tokio::spawn`
/// task. `GitWorktreeManager` is a trivial wrapper around a `PathBuf` so `Clone`
/// is cheap; `'static` is required to move the clone across the task boundary.
pub trait SpawnableGitWorktree: GitWorktree + Clone + 'static {}
impl<T: GitWorktree + Clone + 'static> SpawnableGitWorktree for T {}

/// Execute all effects for one issue.
///
/// Effects are processed in priority order (already sorted by `Decision::new`).
/// A non-best-effort failure aborts the rest. Best-effort failures are logged only.
#[allow(clippy::too_many_arguments)]
pub async fn execute_effects<G, I, P, C, W, F>(
    github: &G,
    issue_repo: &I,
    process_repo: &P,
    claude_runner: &C,
    worktree: &W,
    file_gen: &F,
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
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    for effect in effects {
        let best_effort = effect.is_best_effort();
        let result = execute_one(
            github,
            issue_repo,
            process_repo,
            claude_runner,
            worktree,
            file_gen,
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
async fn execute_one<G, I, P, C, W, F>(
    github: &G,
    issue_repo: &I,
    process_repo: &P,
    claude_runner: &C,
    worktree: &W,
    file_gen: &F,
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
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
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

        Effect::PostRetryExhaustedComment {
            consecutive_failures,
            process_type,
        } => {
            let unknown = rust_i18n::t!("issue_comment.unknown_error", locale = lang);
            // Find the last error message from the most recent failed process run
            // for the same process type as the exhausted retry count.
            let last_error = find_last_error(process_repo, issue.id, *process_type).await;
            let error_str = last_error.as_deref().unwrap_or(&unknown);
            let count = consecutive_failures;
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
                file_gen,
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

        Effect::SpawnProcess {
            type_,
            causes,
            pending_run_id,
        } => {
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
                *pending_run_id,
            )
            .await?;
        }

        Effect::CleanupWorktree => {
            if let Some(ref wt) = issue.worktree_path {
                let path = Path::new(wt);
                if path.exists() {
                    worktree.remove(path)?;
                }

                // Delete branches (best-effort, idempotent)
                let feature_name = &issue.feature_name;
                let n = issue.github_issue_number;
                for branch in [
                    format!("cupola/{feature_name}/main"),
                    format!("cupola/{feature_name}/design"),
                ] {
                    match worktree.delete_branch(&branch) {
                        Ok(()) => {
                            tracing::info!(
                                issue_number = n,
                                branch = %branch,
                                "branch deleted"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                issue_number = n,
                                branch = %branch,
                                error = %e,
                                "failed to delete branch, skipping"
                            );
                        }
                    }
                }

                // Clear worktree_path in DB, retrying on transient failures so
                // that in-memory state stays consistent with persisted state.
                let updates = MetadataUpdates {
                    worktree_path: Some(None),
                    ..Default::default()
                };
                let issue_id = issue.id;
                retry_db_update(n, "CleanupWorktree", || {
                    issue_repo.update_state_and_metadata(issue_id, &updates)
                })
                .await?;
                issue.worktree_path = None;
            }
        }

        Effect::CloseIssue => {
            github.close_issue(n).await?;

            // Mark close_finished = true, retrying on transient DB failures so
            // that in-memory state stays consistent with the persisted state.
            let updates = MetadataUpdates {
                close_finished: Some(true),
                ..Default::default()
            };
            let issue_id = issue.id;
            retry_db_update(n, "CloseIssue", || {
                issue_repo.update_state_and_metadata(issue_id, &updates)
            })
            .await?;
            issue.close_finished = true;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn spawn_init_task<G, I, P, W, F>(
    github: &G,
    _issue_repo: &I,
    process_repo: &P,
    worktree: &W,
    file_gen: &F,
    init_mgr: &mut InitTaskManager,
    config: &Config,
    issue: &Issue,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    // Atomically claim the slot for this issue_id, preventing a second
    // concurrent invocation from spawning a duplicate task (TOCTOU fix).
    if !init_mgr.try_claim(issue.id) {
        tracing::debug!(
            issue_id = issue.id,
            "init task already active or pending, skipping"
        );
        return Ok(());
    }

    // Perform all async I/O in a helper.  On failure, release the claim so
    // that a future cycle can retry.
    match prepare_init_handle(github, process_repo, worktree, file_gen, config, issue).await {
        Ok(handle) => {
            // register() consumes the pending claim.
            init_mgr.register(issue.id, handle);
        }
        Err(e) => {
            init_mgr.release_claim(issue.id);
            return Err(e);
        }
    }
    Ok(())
}

/// Performs all async I/O for an init task and returns the spawned JoinHandle.
///
/// Separated from `spawn_init_task` so that the claim lifecycle (try_claim /
/// release_claim / register) stays in the caller with no scattered `?` returns.
async fn prepare_init_handle<G, P, W, F>(
    github: &G,
    process_repo: &P,
    worktree: &W,
    file_gen: &F,
    config: &Config,
    issue: &Issue,
) -> Result<tokio::task::JoinHandle<anyhow::Result<String>>>
where
    G: GitHubClient,
    P: ProcessRunRepository,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    // Get next index.
    let latest = process_repo
        .find_latest(issue.id, ProcessRunType::Init)
        .await?;
    let index = latest.map(|r| r.index + 1).unwrap_or(0);

    // Fetch issue detail *before* inserting a ProcessRun so that a GitHub
    // failure here does not leave an Init run stuck in `running` state.
    // If get_issue fails, decide() will retry SpawnInit next cycle cleanly.
    let detail = github.get_issue(issue.github_issue_number).await?;

    // Insert ProcessRun(init, state=running). Resolve will transition it to
    // succeeded/failed when the JoinHandle completes.
    let run = ProcessRun::new_running(issue.id, ProcessRunType::Init, index, vec![]);
    let run_id = process_repo.save(&run).await?;

    let feature_name = issue.feature_name.clone();
    let worktree_base = config
        .log_dir
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("worktrees");
    let wt_path = worktree_base.join(&feature_name);
    let default_branch = config.default_branch.clone();
    let issue_number = issue.github_issue_number;
    let issue_body = detail.body;
    let language = config.language.clone();
    let worktree_clone = worktree.clone();
    let file_gen_clone = file_gen.clone();

    // Check if the worktree already exists (Cancelled → reopen resume case).
    let is_resume = worktree.exists(&wt_path);

    // Post the appropriate comment before starting the blocking git work.
    // On failure, mark the run as failed so it doesn't remain stuck in `running`.
    let lang = &config.language;
    let comment = if is_resume {
        rust_i18n::t!("issue_comment.resuming_design", locale = lang)
    } else {
        rust_i18n::t!("issue_comment.design_starting", locale = lang)
    };
    if let Err(e) = github.comment_on_issue(issue_number, &comment).await {
        let _ = process_repo.mark_failed(run_id, Some(e.to_string())).await;
        return Err(e);
    }

    // tokio::spawn an async task that performs the git work on a blocking thread.
    // Resolve picks up the JoinHandle next cycle and updates the DB — per
    // docs/architecture/effects.md SpawnInit ("実行方法: tokio::spawn").
    let handle = tokio::spawn(async move {
        let wt_path_for_task = wt_path.clone();
        tokio::task::spawn_blocking(move || {
            perform_init_sync(
                &worktree_clone,
                &file_gen_clone,
                &wt_path_for_task,
                issue_number,
                &issue_body,
                &language,
                &feature_name,
                &default_branch,
                is_resume,
            )?;
            Ok::<String, anyhow::Error>(wt_path_for_task.to_string_lossy().into_owned())
        })
        .await
        .map_err(|e| anyhow::anyhow!("init blocking task panicked: {e}"))?
    });

    Ok(handle)
}

#[allow(clippy::too_many_arguments)]
fn perform_init_sync<W: GitWorktree, F: FileGenerator>(
    worktree: &W,
    file_gen: &F,
    wt_path: &Path,
    issue_number: u64,
    issue_body: &str,
    language: &str,
    feature_name: &str,
    default_branch: &str,
    is_resume: bool,
) -> Result<()> {
    worktree.fetch()?;

    if is_resume {
        tracing::info!(
            path = %wt_path.display(),
            "worktree already exists; resuming init with idempotent setup steps"
        );
    }

    let main_branch = format!("cupola/{feature_name}/main");
    let start_point = format!("origin/{default_branch}");
    worktree.create(wt_path, &main_branch, &start_point)?;
    worktree.push(wt_path, &main_branch)?;

    let design_branch = format!("cupola/{feature_name}/design");
    worktree.create_branch(wt_path, &design_branch)?;
    worktree.push(wt_path, &design_branch)?;
    file_gen.generate_spec_directory_at(wt_path, issue_number, issue_body, language)?;

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
    pending_run_id: Option<i64>,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
{
    // Atomically reserve a session slot, preventing concurrent spawn_process
    // calls from both passing the limit check before either registers (TOCTOU fix).
    let reserved = if let Some(max) = config.max_concurrent_sessions {
        if !session_mgr.try_reserve(max as usize) {
            tracing::info!(
                issue_number = issue.github_issue_number,
                "concurrent session limit reached, deferring spawn"
            );
            // No session slot available. If this is a new spawn (no pending record),
            // insert a pending ProcessRun so decide sees Pending next cycle instead
            // of the previous Succeeded/Failed state.
            if pending_run_id.is_none() {
                let latest = process_repo.find_latest(issue.id, type_).await?;
                let index = latest.map(|r| r.index + 1).unwrap_or(0);
                let run = ProcessRun::new_pending(issue.id, type_, index, causes.to_vec());
                let run_id = process_repo.save(&run).await?;
                tracing::info!(
                    issue_number = issue.github_issue_number,
                    run_id,
                    type_ = ?type_,
                    "created pending process run (session limit)"
                );
            }
            return Ok(());
        }
        true
    } else {
        false
    };

    // Perform all async I/O in a helper.  On failure, release the reservation
    // so that a future cycle can retry and a slot isn't silently consumed.
    match prepare_process_spawn(
        github,
        process_repo,
        claude_runner,
        worktree,
        config,
        issue,
        type_,
        causes,
        pending_run_id,
    )
    .await
    {
        Ok((child, run_id, pid)) => {
            // register() consumes the pending reservation.
            session_mgr.register(issue.id, issue.state, child);
            session_mgr.update_run_id(issue.id, run_id);
            tracing::info!(
                issue_number = issue.github_issue_number,
                run_id,
                pid,
                type_ = ?type_,
                "spawned process"
            );
        }
        Err(e) => {
            if reserved {
                session_mgr.release_reservation();
            }
            return Err(e);
        }
    }

    Ok(())
}

/// Performs all async I/O for spawning a Claude Code process and returns the
/// child handle along with DB identifiers.
///
/// Separated from `spawn_process` so that the reservation lifecycle
/// (try_reserve / release_reservation / register) stays in the caller with no
/// scattered `?` returns in between.
#[allow(clippy::too_many_arguments)]
async fn prepare_process_spawn<G, P, C, W>(
    github: &G,
    process_repo: &P,
    claude_runner: &C,
    worktree: &W,
    config: &Config,
    issue: &Issue,
    type_: ProcessRunType,
    causes: &[crate::domain::fixing_problem_kind::FixingProblemKind],
    pending_run_id: Option<i64>,
) -> Result<(Child, i64, u32)>
where
    G: GitHubClient,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
{
    let wt_path_str = issue
        .worktree_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("worktree_path is None for SpawnProcess"))?;
    let wt_path = Path::new(wt_path_str);

    // Fetch issue detail for input files
    let detail = github.get_issue(issue.github_issue_number).await?;

    // Clear and write input files
    clear_inputs_dir(wt_path)?;
    write_issue_input(wt_path, &detail)?;

    // For fixing phases: fetch + merge (ImplFix only), write review threads
    let pr_number_opt = get_pr_number_for_type(process_repo, issue.id, type_).await?;
    if matches!(type_, ProcessRunType::ImplFix) {
        // Fetch latest remote refs so the subsequent merge uses a current
        // origin/main rather than a potentially stale local copy.  Fetch
        // failure is non-fatal: log a warning and fall through to merge so
        // that transient network issues do not block the fixing spawn.
        if let Err(e) = worktree.fetch() {
            tracing::warn!(
                error = %e,
                "fetch failed before fixing merge, proceeding with stale origin"
            );
        }

        // Merge latest default branch. Distinguish conflict (expected, Claude
        // Code will resolve) from other failures (ref/IO/state — nothing we
        // can delegate).
        if let Err(e) = worktree.merge(wt_path, &config.default_branch) {
            if e.downcast_ref::<MergeConflictError>().is_some() {
                tracing::warn!(
                    error = %e,
                    "merge conflict detected before fixing spawn, delegating resolution to Claude Code"
                );
            } else {
                tracing::warn!(
                    error = %e,
                    "merge failed before fixing spawn (non-conflict), proceeding anyway"
                );
            }
        }
    }
    // Write review threads for all fixing types (DesignFix + ImplFix)
    if matches!(type_, ProcessRunType::DesignFix | ProcessRunType::ImplFix)
        && let Some(pr_number) = pr_number_opt
    {
        let threads = github.list_unresolved_threads(pr_number).await?;
        write_review_threads_input(wt_path, &threads, config)?;
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
    )?;

    let schema = match session_config.output_schema {
        OutputSchemaKind::PrCreation => Some(PR_CREATION_SCHEMA),
        OutputSchemaKind::Fixing => Some(FIXING_SCHEMA),
    };

    // Select model
    let model_phase = phase_for_type(type_);
    let model = config.models.resolve(issue.weight, model_phase);

    // Determine run_id: reuse pending or create new running record
    let run_id = if let Some(existing_run_id) = pending_run_id {
        // Resume pending: update state to running
        process_repo
            .update_state(existing_run_id, ProcessRunState::Running)
            .await?;
        existing_run_id
    } else {
        // New spawn: insert as running
        let latest = process_repo.find_latest(issue.id, type_).await?;
        let index = latest.map(|r| r.index + 1).unwrap_or(0);
        let run = ProcessRun::new_running(issue.id, type_, index, causes.to_vec());
        process_repo.save(&run).await?
    };

    // Spawn the process
    match claude_runner.spawn(&session_config.prompt, wt_path, schema, model) {
        Ok(mut child) => {
            let pid = child.id();
            if let Err(e) = process_repo.update_pid(run_id, pid).await {
                let _ = child.kill();
                let _ = child.wait();
                let _ = process_repo
                    .mark_failed(run_id, Some(format!("update_pid failed: {e}")))
                    .await;
                return Err(e);
            }
            Ok((child, run_id, pid))
        }
        Err(e) => {
            let _ = process_repo.mark_failed(run_id, Some(e.to_string())).await;
            Err(e)
        }
    }
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
    process_type: crate::domain::process_run::ProcessRunType,
) -> Option<String> {
    let runs = process_repo.find_by_issue(issue_id).await.ok()?;
    runs.into_iter()
        .rev()
        .find(|r| {
            r.state == crate::domain::process_run::ProcessRunState::Failed
                && r.type_ == process_type
        })
        .and_then(|r| r.error_message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use anyhow::Result;

    use super::{perform_init_sync, prepare_init_handle, prepare_process_spawn};
    use crate::application::port::file_generator::FileGenerator;
    use crate::application::port::git_worktree::GitWorktree;
    use crate::application::port::github_client::{
        GitHubIssueDetail, GitHubPr, GitHubPrDetails, RepositoryPermission, ReviewThread,
    };
    use crate::application::port::process_run_repository::ProcessRunRepository;
    use crate::domain::config::Config;
    use crate::domain::effect::Effect;
    use crate::domain::fixing_problem_kind::FixingProblemKind;
    use crate::domain::issue::Issue;
    use crate::domain::process_run::ProcessRunType;
    use crate::domain::process_run::{ProcessRun, ProcessRunState};

    /// Shared, ordered call log used by both `MockGitWorktree` and
    /// `MockFileGenerator` so tests can assert the full sequence of init
    /// side effects (git ops + spec generation) in one place.
    type CallLog = Arc<Mutex<Vec<String>>>;

    #[derive(Clone, Default)]
    struct MockGitWorktree {
        calls: CallLog,
        worktree_exists: bool,
        fail_fetch: bool,
    }

    impl MockGitWorktree {
        fn with_log(calls: CallLog) -> Self {
            Self {
                calls,
                worktree_exists: false,
                fail_fetch: false,
            }
        }

        fn with_existing_worktree(calls: CallLog) -> Self {
            Self {
                calls,
                worktree_exists: true,
                fail_fetch: false,
            }
        }

        fn with_fail_fetch(calls: CallLog) -> Self {
            Self {
                calls,
                worktree_exists: false,
                fail_fetch: true,
            }
        }
    }

    impl GitWorktree for MockGitWorktree {
        fn fetch(&self) -> Result<()> {
            self.calls.lock().expect("lock").push("fetch".to_string());
            if self.fail_fetch {
                return Err(anyhow::anyhow!("simulated fetch failure"));
            }
            Ok(())
        }

        fn merge(&self, _p: &Path, _b: &str) -> Result<()> {
            self.calls.lock().expect("lock").push("merge".to_string());
            Ok(())
        }

        fn exists(&self, _p: &Path) -> bool {
            self.worktree_exists
        }

        fn create(&self, p: &Path, b: &str, s: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock")
                .push(format!("create:{}:{}:{}", p.display(), b, s));
            Ok(())
        }

        fn remove(&self, _p: &Path) -> Result<()> {
            Ok(())
        }

        fn create_branch(&self, p: &Path, b: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock")
                .push(format!("create_branch:{}:{}", p.display(), b));
            Ok(())
        }

        fn checkout(&self, _p: &Path, _b: &str) -> Result<()> {
            Ok(())
        }

        fn pull(&self, _p: &Path) -> Result<()> {
            Ok(())
        }

        fn push(&self, p: &Path, b: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock")
                .push(format!("push:{}:{}", p.display(), b));
            Ok(())
        }

        fn delete_branch(&self, _b: &str) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct MockFileGenerator {
        calls: CallLog,
    }

    impl MockFileGenerator {
        fn with_log(calls: CallLog) -> Self {
            Self { calls }
        }
    }

    impl FileGenerator for MockFileGenerator {
        fn generate_toml_template(&self) -> Result<bool> {
            Ok(false)
        }

        fn install_claude_code_assets(&self, _upgrade: bool) -> Result<bool> {
            Ok(false)
        }

        fn append_gitignore_entries(&self, _upgrade: bool) -> Result<bool> {
            Ok(false)
        }

        fn generate_spec_directory(
            &self,
            _issue_number: u64,
            _issue_body: &str,
            _language: &str,
        ) -> Result<bool> {
            Ok(false)
        }

        fn generate_spec_directory_at(
            &self,
            base_dir: &Path,
            issue_number: u64,
            issue_body: &str,
            language: &str,
        ) -> Result<bool> {
            self.calls.lock().expect("lock").push(format!(
                "generate_spec_directory_at:{}:{}:{}:{}",
                base_dir.display(),
                issue_number,
                issue_body,
                language,
            ));
            Ok(true)
        }
    }

    /// T-3.EX.1: effects are executed in priority order
    #[test]
    fn effect_priority_order() {
        let mut effects = [
            Effect::CloseIssue,
            Effect::PostCompletedComment,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: None,
            },
        ];
        effects.sort_by_key(|e| e.priority());
        assert_eq!(effects[0], Effect::PostCompletedComment); // priority 1 (transition)
        assert!(matches!(effects[1], Effect::SpawnProcess { .. })); // priority 5
        assert_eq!(effects[2], Effect::CloseIssue); // priority 7
    }

    /// T-3.EX.2: best-effort effects are correctly identified
    #[test]
    fn best_effort_classification() {
        assert!(Effect::PostCompletedComment.is_best_effort());
        assert!(Effect::PostCancelComment.is_best_effort());
        assert!(
            Effect::PostRetryExhaustedComment {
                process_type: ProcessRunType::Design,
                consecutive_failures: 3,
            }
            .is_best_effort()
        );
        assert!(Effect::RejectUntrustedReadyIssue.is_best_effort());
        assert!(Effect::PostCiFixLimitComment.is_best_effort());
        assert!(Effect::CleanupWorktree.is_best_effort());
        assert!(Effect::CloseIssue.is_best_effort());

        assert!(!Effect::SpawnInit.is_best_effort());
        assert!(!Effect::SwitchToImplBranch.is_best_effort());
        assert!(
            !Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: None,
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
            pending_run_id: None,
        };
        if let Effect::SpawnProcess {
            type_: t,
            causes: c,
            ..
        } = effect
        {
            assert_eq!(t, ProcessRunType::DesignFix);
            assert_eq!(c, causes);
        } else {
            panic!("expected SpawnProcess");
        }
    }

    #[test]
    fn perform_init_sync_pushes_branches_and_generates_spec_in_worktree() {
        let log: CallLog = Arc::new(Mutex::new(Vec::new()));
        let worktree = MockGitWorktree::with_log(log.clone());
        let file_gen = MockFileGenerator::with_log(log.clone());
        let wt_path = Path::new("/tmp/cupola-worktree");

        perform_init_sync(
            &worktree,
            &file_gen,
            wt_path,
            193,
            "Issue body",
            "ja",
            "issue-193",
            "main",
            false,
        )
        .expect("perform init");

        // Single shared call log pins the complete ordering of init side
        // effects: fetch -> create main -> push main -> create design ->
        // push design -> spec scaffold. Using one log (instead of per-mock
        // logs) prevents a reorder between git ops and spec generation from
        // slipping through unnoticed.
        assert_eq!(
            log.lock().expect("lock").clone(),
            vec![
                "fetch".to_string(),
                format!(
                    "create:{}:cupola/issue-193/main:origin/main",
                    wt_path.display()
                ),
                format!("push:{}:cupola/issue-193/main", wt_path.display()),
                format!(
                    "create_branch:{}:cupola/issue-193/design",
                    wt_path.display()
                ),
                format!("push:{}:cupola/issue-193/design", wt_path.display()),
                format!(
                    "generate_spec_directory_at:{}:193:Issue body:ja",
                    wt_path.display()
                ),
            ]
        );
    }

    /// T-3.EX.resume: when worktree already exists (is_resume=true), all idempotent
    /// setup steps still run — same sequence as fresh init.  worktree.create()
    /// and create_branch() are already idempotent in the real impl, so re-running
    /// them on resume is safe and avoids skipping required setup for partial inits.
    #[test]
    fn perform_init_sync_resume_runs_idempotent_steps() {
        let log: CallLog = Arc::new(Mutex::new(Vec::new()));
        let worktree = MockGitWorktree::with_existing_worktree(log.clone());
        let file_gen = MockFileGenerator::with_log(log.clone());
        let wt_path = Path::new("/tmp/cupola-worktree");

        perform_init_sync(
            &worktree,
            &file_gen,
            wt_path,
            193,
            "Issue body",
            "ja",
            "issue-193",
            "main",
            true,
        )
        .expect("perform init resume");

        // On resume all idempotent setup steps execute — same sequence as fresh init.
        assert_eq!(
            log.lock().expect("lock").clone(),
            vec![
                "fetch".to_string(),
                format!(
                    "create:{}:cupola/issue-193/main:origin/main",
                    wt_path.display()
                ),
                format!("push:{}:cupola/issue-193/main", wt_path.display()),
                format!(
                    "create_branch:{}:cupola/issue-193/design",
                    wt_path.display()
                ),
                format!("push:{}:cupola/issue-193/design", wt_path.display()),
                format!(
                    "generate_spec_directory_at:{}:193:Issue body:ja",
                    wt_path.display()
                ),
            ]
        );
    }

    // ── Mocks for prepare_init_handle tests ──────────────────────────────────

    #[derive(Clone)]
    struct MockGitHubForInit {
        comments: Arc<Mutex<Vec<String>>>,
        fail_comment: bool,
    }

    impl MockGitHubForInit {
        fn new() -> Self {
            Self {
                comments: Arc::new(Mutex::new(vec![])),
                fail_comment: false,
            }
        }

        fn with_failing_comment(mut self) -> Self {
            self.fail_comment = true;
            self
        }

        fn posted_comments(&self) -> Vec<String> {
            self.comments.lock().expect("lock").clone()
        }
    }

    impl crate::application::port::github_client::GitHubClient for MockGitHubForInit {
        async fn get_issue(&self, number: u64) -> anyhow::Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number,
                title: "Test Issue".to_string(),
                body: "Issue body".to_string(),
                labels: vec![],
            })
        }

        async fn comment_on_issue(&self, _: u64, body: &str) -> anyhow::Result<()> {
            if self.fail_comment {
                return Err(anyhow::anyhow!("comment API error"));
            }
            self.comments.lock().expect("lock").push(body.to_string());
            Ok(())
        }

        async fn list_open_issues(
            &self,
        ) -> anyhow::Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }

        async fn find_pr_by_branches(&self, _: &str, _: &str) -> anyhow::Result<Option<GitHubPr>> {
            Ok(None)
        }

        async fn is_pr_merged(&self, _: u64) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn list_unresolved_threads(&self, _: u64) -> anyhow::Result<Vec<ReviewThread>> {
            Ok(vec![])
        }

        async fn create_pr(&self, _: &str, _: &str, _: &str, _: &str) -> anyhow::Result<u64> {
            Ok(0)
        }

        async fn reply_to_thread(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn resolve_thread(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn close_issue(&self, _: u64) -> anyhow::Result<()> {
            Ok(())
        }

        async fn get_job_logs(&self, _: u64) -> anyhow::Result<String> {
            Ok(String::new())
        }

        async fn get_pr_details(&self, _: u64) -> anyhow::Result<GitHubPrDetails> {
            unimplemented!()
        }

        async fn fetch_label_actor_login(&self, _: u64, _: &str) -> anyhow::Result<Option<String>> {
            Ok(None)
        }

        async fn fetch_user_permission(&self, _: &str) -> anyhow::Result<RepositoryPermission> {
            Ok(RepositoryPermission::Read)
        }

        async fn remove_label(&self, _: u64, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn observe_pr(
            &self,
            _: u64,
        ) -> anyhow::Result<Option<crate::application::port::github_client::PrObservation>>
        {
            Ok(None)
        }
    }

    type FailedRunLog = Arc<Mutex<Vec<(i64, Option<String>)>>>;

    #[derive(Clone)]
    struct MockProcRepo {
        mark_failed_calls: FailedRunLog,
    }

    impl MockProcRepo {
        fn new() -> Self {
            Self {
                mark_failed_calls: Arc::new(Mutex::new(vec![])),
            }
        }

        fn failed_runs(&self) -> Vec<(i64, Option<String>)> {
            self.mark_failed_calls.lock().expect("lock").clone()
        }
    }

    impl ProcessRunRepository for MockProcRepo {
        async fn save(&self, _: &ProcessRun) -> anyhow::Result<i64> {
            Ok(42)
        }

        async fn find_latest(
            &self,
            _: i64,
            _: ProcessRunType,
        ) -> anyhow::Result<Option<ProcessRun>> {
            Ok(None)
        }

        async fn find_latest_with_pr_number(
            &self,
            _: i64,
            _: ProcessRunType,
        ) -> anyhow::Result<Option<ProcessRun>> {
            Ok(None)
        }

        async fn mark_failed(&self, run_id: i64, msg: Option<String>) -> anyhow::Result<()> {
            self.mark_failed_calls
                .lock()
                .expect("lock")
                .push((run_id, msg));
            Ok(())
        }

        async fn update_pid(&self, _: i64, _: u32) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_succeeded(&self, _: i64, _: Option<u64>) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_stale(&self, _: i64) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_stale_for_issue(&self, _: i64) -> anyhow::Result<()> {
            Ok(())
        }

        async fn find_by_issue(&self, _: i64) -> anyhow::Result<Vec<ProcessRun>> {
            Ok(vec![])
        }

        async fn count_consecutive_failures(
            &self,
            _: i64,
            _: ProcessRunType,
            _: Option<chrono::DateTime<chrono::Utc>>,
        ) -> anyhow::Result<u32> {
            Ok(0)
        }

        async fn find_latest_with_consecutive_count(
            &self,
            _: i64,
            _: ProcessRunType,
            _: Option<chrono::DateTime<chrono::Utc>>,
        ) -> anyhow::Result<Option<(ProcessRun, u32)>> {
            Ok(None)
        }

        async fn find_all_running(&self) -> anyhow::Result<Vec<ProcessRun>> {
            Ok(vec![])
        }

        async fn update_state(&self, _: i64, _: ProcessRunState) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn make_test_issue(feature_name: &str) -> Issue {
        let mut issue = Issue::new(1, feature_name.to_string());
        issue.id = 1;
        issue
    }

    fn make_test_config(worktrees_parent: &std::path::Path) -> Config {
        let mut config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        // log_dir parent becomes the worktrees_parent; wt_path = parent/worktrees/<feature>
        config.log_dir = worktrees_parent.join("logs").join("cupola.log");
        config
    }

    /// T-3.EX.ph.fresh: prepare_init_handle posts `design_starting` comment when
    /// the worktree does not yet exist.
    #[tokio::test]
    async fn prepare_init_handle_posts_design_starting_on_fresh_init() {
        let dir = tempfile::tempdir().unwrap();
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktree::with_log(Arc::new(Mutex::new(vec![])));
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let issue = make_test_issue("issue-1");
        let config = make_test_config(dir.path());

        prepare_init_handle(&github, &proc_repo, &worktree, &file_gen, &config, &issue)
            .await
            .expect("prepare_init_handle should succeed");

        let comments = github.posted_comments();
        assert_eq!(comments.len(), 1);
        // The comment body is an i18n string — just verify it is not the resuming variant.
        assert!(
            !comments[0].contains("resuming"),
            "fresh init should not post a resuming comment, got: {:?}",
            comments[0]
        );
    }

    /// T-3.EX.ph.resume: prepare_init_handle posts `resuming_design` comment when
    /// the worktree path already exists (Cancelled → reopen scenario).
    #[tokio::test]
    async fn prepare_init_handle_posts_resuming_design_on_existing_worktree() {
        let dir = tempfile::tempdir().unwrap();
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        // worktree_exists=true simulates the path already present on disk.
        let worktree = MockGitWorktree::with_existing_worktree(Arc::new(Mutex::new(vec![])));
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let issue = make_test_issue("issue-1");
        let config = make_test_config(dir.path());

        prepare_init_handle(&github, &proc_repo, &worktree, &file_gen, &config, &issue)
            .await
            .expect("prepare_init_handle should succeed");

        let comments = github.posted_comments();
        assert_eq!(comments.len(), 1);
        // Config defaults to `language = "ja"`, so the resuming_design key
        // yields "設計を再開します". Check for the resume marker "再開".
        assert!(
            comments[0].contains("再開"),
            "resume init should post a resuming comment (再開), got: {:?}",
            comments[0]
        );
    }

    /// T-3.EX.ph.comment-fail: when comment_on_issue fails, prepare_init_handle
    /// marks the ProcessRun as failed and propagates the error.
    #[tokio::test]
    async fn prepare_init_handle_marks_run_failed_on_comment_error() {
        let dir = tempfile::tempdir().unwrap();
        let github = MockGitHubForInit::new().with_failing_comment();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktree::with_log(Arc::new(Mutex::new(vec![])));
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let issue = make_test_issue("issue-1");
        let config = make_test_config(dir.path());

        let result =
            prepare_init_handle(&github, &proc_repo, &worktree, &file_gen, &config, &issue).await;

        assert!(result.is_err(), "should propagate comment error");
        let failed = proc_repo.failed_runs();
        assert_eq!(failed.len(), 1, "should mark exactly one run as failed");
        assert_eq!(failed[0].0, 42, "should use the saved run_id");
        assert!(
            failed[0]
                .1
                .as_deref()
                .unwrap_or("")
                .contains("comment API error"),
            "error message should be propagated"
        );
    }

    // ── Mocks for CleanupWorktree tests ──────────────────────────────────────

    #[derive(Clone)]
    struct MockGitWorktreeForCleanup {
        deleted_branches: Arc<Mutex<Vec<String>>>,
        fail_delete_branch: bool,
    }

    impl MockGitWorktreeForCleanup {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let deleted = Arc::new(Mutex::new(vec![]));
            (
                Self {
                    deleted_branches: deleted.clone(),
                    fail_delete_branch: false,
                },
                deleted,
            )
        }

        fn with_failing_delete(mut self) -> Self {
            self.fail_delete_branch = true;
            self
        }
    }

    impl GitWorktree for MockGitWorktreeForCleanup {
        fn fetch(&self) -> Result<()> {
            Ok(())
        }
        fn merge(&self, _: &Path, _: &str) -> Result<()> {
            Ok(())
        }
        fn exists(&self, _: &Path) -> bool {
            false
        }
        fn create(&self, _: &Path, _: &str, _: &str) -> Result<()> {
            Ok(())
        }
        fn remove(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn create_branch(&self, _: &Path, _: &str) -> Result<()> {
            Ok(())
        }
        fn checkout(&self, _: &Path, _: &str) -> Result<()> {
            Ok(())
        }
        fn pull(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn push(&self, _: &Path, _: &str) -> Result<()> {
            Ok(())
        }
        fn delete_branch(&self, branch: &str) -> Result<()> {
            if self.fail_delete_branch {
                return Err(anyhow::anyhow!("delete_branch failed"));
            }
            self.deleted_branches
                .lock()
                .expect("lock")
                .push(branch.to_string());
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MockIssueRepoForCleanup {
        metadata_updates: Arc<Mutex<Vec<crate::domain::metadata_update::MetadataUpdates>>>,
    }

    impl MockIssueRepoForCleanup {
        fn new() -> (
            Self,
            Arc<Mutex<Vec<crate::domain::metadata_update::MetadataUpdates>>>,
        ) {
            let updates = Arc::new(Mutex::new(vec![]));
            (
                Self {
                    metadata_updates: updates.clone(),
                },
                updates,
            )
        }
    }

    impl crate::application::port::issue_repository::IssueRepository for MockIssueRepoForCleanup {
        async fn find_by_id(&self, _: i64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_by_issue_number(&self, _: u64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_active(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn find_all(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn save(&self, _: &Issue) -> Result<i64> {
            unimplemented!()
        }
        async fn update_state(&self, _: i64, _: crate::domain::state::State) -> Result<()> {
            unimplemented!()
        }
        async fn update(&self, _: &Issue) -> Result<()> {
            unimplemented!()
        }
        async fn update_state_and_metadata(
            &self,
            _: i64,
            updates: &crate::domain::metadata_update::MetadataUpdates,
        ) -> Result<()> {
            self.metadata_updates
                .lock()
                .expect("lock")
                .push(updates.clone());
            Ok(())
        }
        async fn find_by_state(&self, _: crate::domain::state::State) -> Result<Vec<Issue>> {
            unimplemented!()
        }
    }

    struct MockClaudeRunner;

    impl crate::application::port::claude_code_runner::ClaudeCodeRunner for MockClaudeRunner {
        fn spawn(
            &self,
            _: &str,
            _: &Path,
            _: Option<&str>,
            _: &str,
        ) -> Result<std::process::Child> {
            unimplemented!()
        }
    }

    /// `ClaudeCodeRunner` that always returns an error from `spawn`.
    /// Used in `prepare_process_spawn` tests so we can observe git side-effects
    /// without needing a real process to be created.
    struct MockClaudeRunnerFailing;

    impl crate::application::port::claude_code_runner::ClaudeCodeRunner for MockClaudeRunnerFailing {
        fn spawn(
            &self,
            _: &str,
            _: &Path,
            _: Option<&str>,
            _: &str,
        ) -> Result<std::process::Child> {
            Err(anyhow::anyhow!("spawn intentionally failed in test"))
        }
    }

    fn make_completed_issue_with_worktree(feature_name: &str, wt_path: &str) -> Issue {
        Issue {
            id: 1,
            github_issue_number: 42,
            state: crate::domain::state::State::Completed,
            feature_name: feature_name.to_string(),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            worktree_path: Some(wt_path.to_string()),
            ci_fix_count: 0,
            close_finished: false,
            consecutive_failures_epoch: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// T-3.CW.1: CleanupWorktree deletes both branches after successful worktree removal
    #[tokio::test]
    async fn cleanup_worktree_deletes_both_branches() {
        let tmp = tempfile::tempdir().unwrap();
        let wt_path = tmp.path().to_string_lossy().to_string();
        let mut issue = make_completed_issue_with_worktree("issue-190", &wt_path);

        let (worktree, deleted) = MockGitWorktreeForCleanup::new();
        let (issue_repo, metadata_updates) = MockIssueRepoForCleanup::new();
        let proc_repo = MockProcRepo::new();
        let github = MockGitHubForInit::new();
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());

        let effects = [Effect::CleanupWorktree];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed");

        let branches = deleted.lock().expect("lock");
        assert!(
            branches.contains(&"cupola/issue-190/main".to_string()),
            "should delete main branch"
        );
        assert!(
            branches.contains(&"cupola/issue-190/design".to_string()),
            "should delete design branch"
        );

        let updates = metadata_updates.lock().expect("lock");
        assert_eq!(updates.len(), 1, "should update metadata once");
        assert!(
            updates[0].worktree_path == Some(None),
            "should clear worktree_path"
        );
        assert!(issue.worktree_path.is_none(), "issue.worktree_path cleared");
    }

    /// T-3.CW.2: CleanupWorktree continues and clears metadata even when delete_branch fails
    #[tokio::test]
    async fn cleanup_worktree_continues_on_delete_branch_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let wt_path = tmp.path().to_string_lossy().to_string();
        let mut issue = make_completed_issue_with_worktree("issue-190", &wt_path);

        let (worktree, _deleted) = MockGitWorktreeForCleanup::new();
        let worktree = worktree.with_failing_delete();
        let (issue_repo, metadata_updates) = MockIssueRepoForCleanup::new();
        let proc_repo = MockProcRepo::new();
        let github = MockGitHubForInit::new();
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());

        let effects = [Effect::CleanupWorktree];
        let result = super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await;

        assert!(result.is_ok(), "should not fail when delete_branch fails");
        let updates = metadata_updates.lock().expect("lock");
        assert_eq!(
            updates.len(),
            1,
            "should still clear metadata despite branch deletion failure"
        );
    }

    /// T-3.CW.3: CleanupWorktree skips branch deletion when worktree_path is None
    #[tokio::test]
    async fn cleanup_worktree_skips_branch_deletion_when_no_worktree_path() {
        let mut issue = Issue {
            id: 1,
            github_issue_number: 42,
            state: crate::domain::state::State::Completed,
            feature_name: "issue-190".to_string(),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            worktree_path: None,
            ci_fix_count: 0,
            close_finished: false,
            consecutive_failures_epoch: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let (worktree, deleted) = MockGitWorktreeForCleanup::new();
        let (issue_repo, metadata_updates) = MockIssueRepoForCleanup::new();
        let proc_repo = MockProcRepo::new();
        let github = MockGitHubForInit::new();
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());

        let effects = [Effect::CleanupWorktree];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed");

        assert!(
            deleted.lock().expect("lock").is_empty(),
            "should not call delete_branch when worktree_path is None"
        );
        assert!(
            metadata_updates.lock().expect("lock").is_empty(),
            "should not update metadata when worktree_path is None"
        );
    }

    // ── Tests for PostRetryExhaustedComment payload usage ────────────────────

    /// T-3.EX.re.1: PostRetryExhaustedComment uses consecutive_failures from payload as count
    #[tokio::test]
    async fn retry_exhausted_comment_uses_payload_consecutive_failures() {
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let (issue_repo, _) = MockIssueRepoForCleanup::new();
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        let mut issue = make_test_issue("issue-1");

        let effects = [Effect::PostRetryExhaustedComment {
            process_type: ProcessRunType::Design,
            consecutive_failures: 3,
        }];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed");

        let comments = github.posted_comments();
        assert_eq!(comments.len(), 1, "should post exactly one comment");
        // The default locale is "ja", so the retry_exhausted template renders
        // the count as "（3 回）". Assert on this specific substring to ensure
        // the interpolated count value is exactly 3, not any incidental "3".
        assert!(
            comments[0].contains("（3 回）"),
            "comment should contain the consecutive_failures count formatted as '（3 回）', got: {:?}",
            comments[0]
        );
    }

    // ── Tests for prepare_process_spawn fetch-before-merge behaviour ─────────

    fn make_fixing_issue(wt_path: &str) -> Issue {
        let mut issue = Issue::new(42, "issue-42".to_string());
        issue.id = 1;
        issue.state = crate::domain::state::State::DesignFixing;
        issue.worktree_path = Some(wt_path.to_string());
        issue
    }

    /// T-fix.1: ImplFix spawn で fetch → merge の順序が保証されること（要件 2.1）
    #[tokio::test]
    async fn impl_fixing_spawn_calls_fetch_before_merge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wt_path = dir.path().to_str().expect("utf8");
        let mut issue = make_fixing_issue(wt_path);
        issue.state = crate::domain::state::State::ImplementationFixing;
        let config = make_test_config(dir.path());

        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log.clone());
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();

        let _ = prepare_process_spawn(
            &github,
            &proc_repo,
            &MockClaudeRunnerFailing,
            &worktree,
            &config,
            &issue,
            ProcessRunType::ImplFix,
            &[],
            None,
        )
        .await;

        let calls = log.lock().expect("lock").clone();
        let fetch_pos = calls.iter().position(|c| c == "fetch");
        let merge_pos = calls.iter().position(|c| c == "merge");

        assert!(
            fetch_pos.is_some(),
            "fetch should be called in ImplFix spawn, calls: {:?}",
            calls
        );
        assert!(
            merge_pos.is_some(),
            "merge should be called in ImplFix spawn, calls: {:?}",
            calls
        );
        assert!(
            fetch_pos.unwrap() < merge_pos.unwrap(),
            "fetch must be called before merge, calls: {:?}",
            calls
        );
    }

    /// T-fix.2: ImplFix で fetch が失敗しても merge が呼ばれること（要件 2.2）
    #[tokio::test]
    async fn impl_fixing_spawn_continues_to_merge_when_fetch_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wt_path = dir.path().to_str().expect("utf8");
        let mut issue = make_fixing_issue(wt_path);
        issue.state = crate::domain::state::State::ImplementationFixing;
        let config = make_test_config(dir.path());

        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_fail_fetch(log.clone());
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();

        let _ = prepare_process_spawn(
            &github,
            &proc_repo,
            &MockClaudeRunnerFailing,
            &worktree,
            &config,
            &issue,
            ProcessRunType::ImplFix,
            &[],
            None,
        )
        .await;

        let calls = log.lock().expect("lock").clone();
        assert!(
            calls.contains(&"fetch".to_string()),
            "fetch should be attempted even when it fails, calls: {:?}",
            calls
        );
        assert!(
            calls.contains(&"merge".to_string()),
            "merge should be called even when fetch fails, calls: {:?}",
            calls
        );
    }

    /// T-fix.3: Design などの non-fixing spawn type では fetch が呼ばれないこと（要件 2.3）
    #[tokio::test]
    async fn non_fixing_spawn_does_not_call_fetch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wt_path = dir.path().to_str().expect("utf8");

        // Design spawn type (non-fixing)
        let mut issue = make_fixing_issue(wt_path);
        issue.state = crate::domain::state::State::DesignRunning;
        let config = make_test_config(dir.path());

        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log.clone());
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();

        let _ = prepare_process_spawn(
            &github,
            &proc_repo,
            &MockClaudeRunnerFailing,
            &worktree,
            &config,
            &issue,
            ProcessRunType::Design,
            &[],
            None,
        )
        .await;

        let calls = log.lock().expect("lock").clone();
        assert!(
            !calls.contains(&"fetch".to_string()),
            "fetch should NOT be called for non-fixing spawn types, calls: {:?}",
            calls
        );
    }

    /// T-fix.4: DesignFix spawn では fetch/merge が呼ばれないこと
    #[tokio::test]
    async fn design_fixing_spawn_does_not_call_fetch_or_merge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wt_path = dir.path().to_str().expect("utf8");
        let issue = make_fixing_issue(wt_path);
        let config = make_test_config(dir.path());

        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log.clone());
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();

        let _ = prepare_process_spawn(
            &github,
            &proc_repo,
            &MockClaudeRunnerFailing,
            &worktree,
            &config,
            &issue,
            ProcessRunType::DesignFix,
            &[],
            None,
        )
        .await;

        let calls = log.lock().expect("lock").clone();
        assert!(
            !calls.contains(&"fetch".to_string()),
            "fetch should NOT be called for DesignFix spawn, calls: {:?}",
            calls
        );
        assert!(
            !calls.contains(&"merge".to_string()),
            "merge should NOT be called for DesignFix spawn, calls: {:?}",
            calls
        );
    }

    // ── Mock IssueRepository for retry tests ────────────────────────────────

    /// IssueRepository mock that fails `update_state_and_metadata` a configurable
    /// number of times before succeeding. All other methods are unimplemented.
    #[derive(Clone)]
    struct MockIssueRepoForRetry {
        call_count: Arc<Mutex<u32>>,
        max_failures: u32,
    }

    impl MockIssueRepoForRetry {
        /// Fails `max_failures` times, then succeeds.
        fn fail_then_succeed(max_failures: u32) -> Self {
            Self {
                call_count: Arc::new(Mutex::new(0)),
                max_failures,
            }
        }

        /// Always fails (max_failures = u32::MAX).
        fn always_fail() -> Self {
            Self::fail_then_succeed(u32::MAX)
        }

        fn call_count(&self) -> u32 {
            *self.call_count.lock().expect("lock")
        }
    }

    impl crate::application::port::issue_repository::IssueRepository for MockIssueRepoForRetry {
        async fn find_by_id(&self, _: i64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_by_issue_number(&self, _: u64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_active(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn find_all(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn save(&self, _: &Issue) -> Result<i64> {
            unimplemented!()
        }
        async fn update_state(&self, _: i64, _: crate::domain::state::State) -> Result<()> {
            unimplemented!()
        }
        async fn update(&self, _: &Issue) -> Result<()> {
            unimplemented!()
        }
        async fn update_state_and_metadata(
            &self,
            _: i64,
            _: &crate::domain::metadata_update::MetadataUpdates,
        ) -> Result<()> {
            let mut count = self.call_count.lock().expect("lock");
            *count += 1;
            let n = *count;
            if n <= self.max_failures {
                Err(anyhow::anyhow!("simulated DB failure (call {})", n))
            } else {
                Ok(())
            }
        }
        async fn find_by_state(&self, _: crate::domain::state::State) -> Result<Vec<Issue>> {
            unimplemented!()
        }
    }

    // ── retry_db_update unit tests ───────────────────────────────────────────

    /// T-4.1: 一時的失敗から回復するケース — 1回失敗後に成功する
    ///
    /// start_paused=true でトキオの時刻を一時停止し、sleep を即時完了させる。
    #[tokio::test(start_paused = true)]
    async fn retry_db_update_recovers_from_transient_failure() {
        let call_count = Arc::new(Mutex::new(0u32));
        let cc = call_count.clone();

        let result = super::retry_db_update(42, "TestEffect", || {
            let mut n = cc.lock().expect("lock");
            *n += 1;
            let attempt = *n;
            async move {
                if attempt == 1 {
                    Err(anyhow::anyhow!("transient failure"))
                } else {
                    Ok(())
                }
            }
        })
        .await;

        assert!(result.is_ok(), "should succeed after one transient failure");
        assert_eq!(
            *call_count.lock().expect("lock"),
            2,
            "should call db_update exactly twice (1 failure + 1 success)"
        );
    }

    /// T-4.2: 回復不能な失敗のケース — 全試行で失敗する
    ///
    /// RETRY_DELAYS.len() + 1 = 4 回呼ばれることを検証する。
    #[tokio::test(start_paused = true)]
    async fn retry_db_update_exhausts_all_retries_and_returns_error() {
        let call_count = Arc::new(Mutex::new(0u32));
        let cc = call_count.clone();

        let result = super::retry_db_update(42, "TestEffect", || {
            let mut n = cc.lock().expect("lock");
            *n += 1;
            async move { Err::<(), _>(anyhow::anyhow!("permanent failure")) }
        })
        .await;

        assert!(
            result.is_err(),
            "should return error after all retries exhausted"
        );
        assert_eq!(
            *call_count.lock().expect("lock"),
            super::RETRY_DELAYS.len() as u32 + 1,
            "should call db_update exactly RETRY_DELAYS.len()+1 times"
        );
    }

    // ── CloseIssue retry integration tests ─────────────────────────────────

    /// T-3.1: CloseIssue で DB 更新が 1 回失敗後に成功したとき close_finished=true になる
    #[tokio::test(start_paused = true)]
    async fn close_issue_sets_close_finished_after_transient_db_failure() {
        let issue_repo = MockIssueRepoForRetry::fail_then_succeed(1);
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        let mut issue = make_test_issue("issue-42");

        let effects = [Effect::CloseIssue];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed (CloseIssue is best-effort)");

        assert!(
            issue.close_finished,
            "close_finished should be true after successful retry"
        );
        assert_eq!(
            issue_repo.call_count(),
            2,
            "update_state_and_metadata should be called twice (1 failure + 1 success)"
        );
    }

    /// T-3.4 (CloseIssue): DB 更新が永続的に失敗したとき close_finished は変更されない
    #[tokio::test(start_paused = true)]
    async fn close_issue_does_not_set_close_finished_on_permanent_db_failure() {
        let issue_repo = MockIssueRepoForRetry::always_fail();
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        let mut issue = make_test_issue("issue-42");

        // CloseIssue is best-effort, so execute_effects returns Ok even on DB failure.
        let effects = [Effect::CloseIssue];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects returns Ok for best-effort effect");

        assert!(
            !issue.close_finished,
            "close_finished must remain false when DB update permanently fails"
        );
        assert_eq!(
            issue_repo.call_count(),
            super::RETRY_DELAYS.len() as u32 + 1,
            "should exhaust all retries"
        );
    }

    // ── CleanupWorktree retry integration tests ─────────────────────────────

    /// T-3.2 / T-3.4 (CleanupWorktree): DB 更新が永続的に失敗したとき worktree_path は変更されない
    #[tokio::test(start_paused = true)]
    async fn cleanup_worktree_does_not_clear_worktree_path_on_permanent_db_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let wt_path_str = tmp.path().to_string_lossy().to_string();
        let mut issue = make_completed_issue_with_worktree("issue-42", &wt_path_str);

        let issue_repo = MockIssueRepoForRetry::always_fail();
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());

        // CleanupWorktree is best-effort, so execute_effects returns Ok even on DB failure.
        let effects = [Effect::CleanupWorktree];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects returns Ok for best-effort effect");

        assert!(
            issue.worktree_path.is_some(),
            "worktree_path must remain Some when DB update permanently fails"
        );
        assert_eq!(
            issue_repo.call_count(),
            super::RETRY_DELAYS.len() as u32 + 1,
            "should exhaust all retries"
        );
    }
}
