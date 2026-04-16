use chrono::Utc;

use crate::domain::config::Config;
use crate::domain::decision::Decision;
use crate::domain::effect::Effect;
use crate::domain::fixing_problem_kind::FixingProblemKind;
use crate::domain::issue::Issue;
use crate::domain::metadata_update::MetadataUpdates;
use crate::domain::process_run::{ProcessRunState, ProcessRunType};
use crate::domain::state::State;
use crate::domain::world_snapshot::{GithubIssueSnapshot, PrState, WorldSnapshot};

/// Pure decision function: given the current Issue state, observations (WorldSnapshot),
/// and configuration, returns the Decision (next_state + metadata_updates + effects).
/// This function performs NO I/O.
pub fn decide(prev: &Issue, snap: &WorldSnapshot, cfg: &Config) -> Decision {
    let mut effects: Vec<Effect> = Vec::new();
    let mut metadata_updates = MetadataUpdates::default();

    // Cross-cutting: propagate weight change (only meaningful for open issues)
    if let GithubIssueSnapshot::Open {
        weight: Some(snap_weight),
        ..
    } = &snap.github_issue
        && *snap_weight != prev.weight
    {
        metadata_updates.weight = Some(*snap_weight);
    }

    let next_state = dispatch_state(prev, snap, cfg, &mut effects, &mut metadata_updates);

    if next_state != prev.state {
        metadata_updates.state = Some(next_state);
    }

    Decision::new(next_state, metadata_updates, effects)
}

/// Internal dispatch helper. Returns the `decide_X` result for `issue.state`.
fn dispatch_state(
    issue: &Issue,
    snap: &WorldSnapshot,
    cfg: &Config,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> State {
    match issue.state {
        State::Idle => decide_idle(issue, snap, cfg, effects, metadata_updates),
        State::InitializeRunning => {
            decide_initialize_running(issue, snap, cfg, effects, metadata_updates)
        }
        State::DesignRunning => decide_design_running(issue, snap, cfg, effects, metadata_updates),
        State::DesignReviewWaiting => {
            decide_design_review_waiting(issue, snap, cfg, effects, metadata_updates)
        }
        State::DesignFixing => decide_design_fixing(issue, snap, cfg, effects, metadata_updates),
        State::ImplementationRunning => {
            decide_implementation_running(issue, snap, cfg, effects, metadata_updates)
        }
        State::ImplementationReviewWaiting => {
            decide_implementation_review_waiting(issue, snap, cfg, effects, metadata_updates)
        }
        State::ImplementationFixing => {
            decide_implementation_fixing(issue, snap, cfg, effects, metadata_updates)
        }
        State::Completed => decide_completed(issue, snap, effects, metadata_updates),
        State::Cancelled => decide_cancelled(issue, snap, effects, metadata_updates),
    }
}

fn issue_is_closed(snap: &WorldSnapshot) -> bool {
    matches!(snap.github_issue, GithubIssueSnapshot::Closed)
}

fn go_cancelled_issue_closed(effects: &mut Vec<Effect>) -> State {
    effects.push(Effect::PostCancelComment);
    effects.push(Effect::CloseIssue);
    State::Cancelled
}

fn go_cancelled_retry_exhausted(
    effects: &mut Vec<Effect>,
    process_type: ProcessRunType,
    consecutive_failures: u32,
) -> State {
    effects.push(Effect::PostRetryExhaustedComment {
        process_type,
        consecutive_failures,
    });
    effects.push(Effect::CloseIssue);
    State::Cancelled
}

/// If the impl PR is Merged, unconditionally transition to Completed.
/// This must be checked *before* `issue_is_closed`, because GitHub auto-closes
/// issues when a linked PR merges — without this short-circuit the next
/// polling cycle would observe Closed and incorrectly cancel a successful run.
fn try_complete_from_impl_merged(
    snap: &WorldSnapshot,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> Option<State> {
    if let Some(impl_pr) = &snap.impl_pr
        && impl_pr.state == PrState::Merged
    {
        effects.push(Effect::PostCompletedComment);
        effects.push(Effect::CleanupWorktree);
        effects.push(Effect::CloseIssue);
        metadata_updates.ci_fix_count = Some(0);
        return Some(State::Completed);
    }
    None
}

fn decide_idle(
    _prev: &Issue,
    snap: &WorldSnapshot,
    _cfg: &Config,
    effects: &mut Vec<Effect>,
    _metadata_updates: &mut MetadataUpdates,
) -> State {
    if let GithubIssueSnapshot::Open {
        has_ready_label: true,
        ready_label_trusted,
        ..
    } = &snap.github_issue
    {
        if *ready_label_trusted {
            emit_spawn_init(snap, effects);
            State::InitializeRunning
        } else {
            effects.push(Effect::RejectUntrustedReadyIssue);
            State::Idle
        }
    } else {
        State::Idle
    }
}

fn decide_initialize_running(
    _prev: &Issue,
    snap: &WorldSnapshot,
    cfg: &Config,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> State {
    // Priority: closed > retry exhausted > failed > smart routing
    if issue_is_closed(snap) {
        return go_cancelled_issue_closed(effects);
    }

    if let Some(init) = &snap.processes.init {
        if init.consecutive_failures >= cfg.max_retries {
            return go_cancelled_retry_exhausted(effects, ProcessRunType::Init, init.consecutive_failures);
        }
        if init.state == ProcessRunState::Failed {
            effects.push(Effect::SpawnInit);
            return State::InitializeRunning;
        }
        if init.state == ProcessRunState::Succeeded {
            // Smart routing
            if let Some(impl_pr) = &snap.impl_pr {
                if impl_pr.state == PrState::Merged {
                    effects.push(Effect::PostCompletedComment);
                    effects.push(Effect::CleanupWorktree);
                    effects.push(Effect::CloseIssue);
                    return State::Completed;
                }
                if impl_pr.state == PrState::Open {
                    return State::ImplementationReviewWaiting;
                }
            }
            if let Some(design_pr) = &snap.design_pr {
                if design_pr.state == PrState::Merged {
                    metadata_updates.ci_fix_count = Some(0);
                    emit_spawn_impl(snap, effects);
                    return State::ImplementationRunning;
                }
                if design_pr.state == PrState::Open {
                    return State::DesignReviewWaiting;
                }
            }
            emit_spawn_design(snap, effects);
            return State::DesignRunning;
        }
        // init.state == Running or Stale: emit SpawnInit if not running
        if init.state != ProcessRunState::Running {
            effects.push(Effect::SpawnInit);
        }
        State::InitializeRunning
    } else {
        // No init process at all - spawn one
        effects.push(Effect::SpawnInit);
        State::InitializeRunning
    }
}

fn decide_design_running(
    _prev: &Issue,
    snap: &WorldSnapshot,
    cfg: &Config,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> State {
    if issue_is_closed(snap) {
        return go_cancelled_issue_closed(effects);
    }

    if let Some(design) = &snap.processes.design {
        if design.consecutive_failures >= cfg.max_retries {
            return go_cancelled_retry_exhausted(effects, ProcessRunType::Design, design.consecutive_failures);
        }
        if design.state == ProcessRunState::Pending {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: Some(design.run_id),
            });
            return State::DesignRunning;
        }
        if design.state == ProcessRunState::Failed {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: None,
            });
            return State::DesignRunning;
        }
        if design.state == ProcessRunState::Stale {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: None,
            });
            return State::DesignRunning;
        }
        if design.state == ProcessRunState::Succeeded {
            if let Some(design_pr) = &snap.design_pr {
                if design_pr.state == PrState::Merged {
                    metadata_updates.ci_fix_count = Some(0);
                    emit_spawn_impl(snap, effects);
                    return State::ImplementationRunning;
                }
                if design_pr.state == PrState::Closed {
                    metadata_updates.ci_fix_count = Some(0);
                    effects.push(Effect::SpawnProcess {
                        type_: ProcessRunType::Design,
                        causes: vec![],
                        pending_run_id: None,
                    });
                    return State::DesignRunning;
                }
                if design_pr.state == PrState::Open {
                    return State::DesignReviewWaiting;
                }
            }
            // Succeeded but PR is None (deleted/404): re-spawn
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: None,
            });
            return State::DesignRunning;
        }
        // Running or other: no spawn
        State::DesignRunning
    } else {
        // No design process: spawn
        effects.push(Effect::SpawnProcess {
            type_: ProcessRunType::Design,
            causes: vec![],
            pending_run_id: None,
        });
        State::DesignRunning
    }
}

fn decide_design_review_waiting(
    prev: &Issue,
    snap: &WorldSnapshot,
    cfg: &Config,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> State {
    if issue_is_closed(snap) {
        return go_cancelled_issue_closed(effects);
    }

    let design_pr = match &snap.design_pr {
        Some(pr) => pr,
        None => {
            emit_spawn_design(snap, effects);
            return State::DesignRunning;
        }
    };

    use crate::domain::world_snapshot::CiStatus;

    // Priority: merge > close > review > ci > conflict
    if design_pr.state == PrState::Merged {
        metadata_updates.ci_fix_count = Some(0);
        emit_spawn_impl(snap, effects);
        return State::ImplementationRunning;
    }
    if design_pr.state == PrState::Closed {
        metadata_updates.ci_fix_count = Some(0);
        emit_spawn_design(snap, effects);
        return State::DesignRunning;
    }
    if design_pr.has_review_comments {
        // review resets ci_fix_count
        metadata_updates.ci_fix_count = Some(0);
        emit_spawn_design_fix(snap, design_pr, effects);
        return State::DesignFixing;
    }
    if design_pr.ci_status == CiStatus::Failure {
        if snap.ci_fix_exhausted {
            // Emit PostCiFixLimitComment only when ci_fix_count == max (not already max+1)
            if prev.ci_fix_count == cfg.max_ci_fix_cycles {
                effects.push(Effect::PostCiFixLimitComment);
                metadata_updates.ci_fix_count = Some(prev.ci_fix_count + 1);
            }
            return State::DesignReviewWaiting;
        }
        metadata_updates.ci_fix_count = Some(prev.ci_fix_count + 1);
        emit_spawn_design_fix(snap, design_pr, effects);
        return State::DesignFixing;
    }
    if design_pr.has_conflict {
        if snap.ci_fix_exhausted {
            if prev.ci_fix_count == cfg.max_ci_fix_cycles {
                effects.push(Effect::PostCiFixLimitComment);
                metadata_updates.ci_fix_count = Some(prev.ci_fix_count + 1);
            }
            return State::DesignReviewWaiting;
        }
        metadata_updates.ci_fix_count = Some(prev.ci_fix_count + 1);
        emit_spawn_design_fix(snap, design_pr, effects);
        return State::DesignFixing;
    }

    State::DesignReviewWaiting
}

fn decide_design_fixing(
    _prev: &Issue,
    snap: &WorldSnapshot,
    cfg: &Config,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> State {
    if issue_is_closed(snap) {
        return go_cancelled_issue_closed(effects);
    }

    let design_pr = match &snap.design_pr {
        Some(pr) => pr,
        None => {
            emit_spawn_design(snap, effects);
            return State::DesignRunning;
        }
    };

    // PR state checks first
    if design_pr.state == PrState::Merged {
        metadata_updates.ci_fix_count = Some(0);
        emit_spawn_impl(snap, effects);
        return State::ImplementationRunning;
    }
    if design_pr.state == PrState::Closed {
        metadata_updates.ci_fix_count = Some(0);
        emit_spawn_design(snap, effects);
        return State::DesignRunning;
    }

    if let Some(design_fix) = &snap.processes.design_fix {
        if design_fix.consecutive_failures >= cfg.max_retries {
            return go_cancelled_retry_exhausted(effects, ProcessRunType::DesignFix, design_fix.consecutive_failures);
        }
        if design_fix.state == ProcessRunState::Pending {
            let causes = derive_fixing_causes(design_pr);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                causes,
                pending_run_id: Some(design_fix.run_id),
            });
            return State::DesignFixing;
        }
        if design_fix.state == ProcessRunState::Failed {
            let causes = derive_fixing_causes(design_pr);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                causes,
                pending_run_id: None,
            });
            return State::DesignFixing;
        }
        if design_fix.state == ProcessRunState::Stale {
            let causes = derive_fixing_causes(design_pr);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                causes,
                pending_run_id: None,
            });
            return State::DesignFixing;
        }
        if design_fix.state == ProcessRunState::Succeeded {
            return State::DesignReviewWaiting;
        }
        // Running or other: stay
        State::DesignFixing
    } else {
        // No design_fix process: spawn
        let causes = derive_fixing_causes(design_pr);
        effects.push(Effect::SpawnProcess {
            type_: ProcessRunType::DesignFix,
            causes,
            pending_run_id: None,
        });
        State::DesignFixing
    }
}

/// Derive causes from PR snapshot (for Fixing states)
fn derive_fixing_causes(pr: &crate::domain::world_snapshot::PrSnapshot) -> Vec<FixingProblemKind> {
    use crate::domain::world_snapshot::CiStatus;
    let mut causes = Vec::new();
    if pr.has_review_comments {
        causes.push(FixingProblemKind::ReviewComments);
    }
    if pr.ci_status == CiStatus::Failure {
        causes.push(FixingProblemKind::CiFailure);
    }
    if pr.has_conflict {
        causes.push(FixingProblemKind::Conflict);
    }
    causes
}

// ── Persistent-effect helpers ──────────────────────────────────────
//
// Each helper emits the persistent effects required when `next.state`
// is the corresponding spawn-capable state, per effects.md.  They are
// called from transition sources that target that state.  The owning
// `decide_*` functions use inline spawn logic for stable cycles.

fn emit_spawn_init(snap: &WorldSnapshot, effects: &mut Vec<Effect>) {
    let dominated = snap
        .processes
        .init
        .as_ref()
        .is_some_and(|p| p.state == ProcessRunState::Running);
    if !dominated {
        effects.push(Effect::SpawnInit);
    }
}

fn emit_spawn_design(snap: &WorldSnapshot, effects: &mut Vec<Effect>) {
    match &snap.processes.design {
        Some(p) if p.state == ProcessRunState::Pending => {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: Some(p.run_id),
            });
        }
        Some(p) if p.state == ProcessRunState::Running => {}
        _ => {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: None,
            });
        }
    }
}

fn emit_spawn_design_fix(
    snap: &WorldSnapshot,
    design_pr: &crate::domain::world_snapshot::PrSnapshot,
    effects: &mut Vec<Effect>,
) {
    let causes = derive_fixing_causes(design_pr);
    match &snap.processes.design_fix {
        Some(p) if p.state == ProcessRunState::Pending => {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                causes,
                pending_run_id: Some(p.run_id),
            });
        }
        Some(p) if p.state == ProcessRunState::Running => {}
        _ => {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                causes,
                pending_run_id: None,
            });
        }
    }
}

fn emit_spawn_impl(snap: &WorldSnapshot, effects: &mut Vec<Effect>) {
    match &snap.processes.impl_ {
        Some(p) if p.state == ProcessRunState::Pending => {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                causes: vec![],
                pending_run_id: Some(p.run_id),
            });
        }
        Some(p) if p.state == ProcessRunState::Running => {}
        _ => {
            effects.push(Effect::SwitchToImplBranch);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                causes: vec![],
                pending_run_id: None,
            });
        }
    }
}

fn emit_spawn_impl_fix(
    snap: &WorldSnapshot,
    impl_pr: &crate::domain::world_snapshot::PrSnapshot,
    effects: &mut Vec<Effect>,
) {
    let causes = derive_fixing_causes(impl_pr);
    match &snap.processes.impl_fix {
        Some(p) if p.state == ProcessRunState::Pending => {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::ImplFix,
                causes,
                pending_run_id: Some(p.run_id),
            });
        }
        Some(p) if p.state == ProcessRunState::Running => {}
        _ => {
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::ImplFix,
                causes,
                pending_run_id: None,
            });
        }
    }
}

fn decide_implementation_running(
    _prev: &Issue,
    snap: &WorldSnapshot,
    cfg: &Config,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> State {
    if let Some(s) = try_complete_from_impl_merged(snap, effects, metadata_updates) {
        return s;
    }
    if issue_is_closed(snap) {
        return go_cancelled_issue_closed(effects);
    }

    if let Some(impl_proc) = &snap.processes.impl_ {
        if impl_proc.consecutive_failures >= cfg.max_retries {
            return go_cancelled_retry_exhausted(effects, ProcessRunType::Impl, impl_proc.consecutive_failures);
        }
        if impl_proc.state == ProcessRunState::Pending {
            // SwitchToImplBranch already done on first attempt
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                causes: vec![],
                pending_run_id: Some(impl_proc.run_id),
            });
            return State::ImplementationRunning;
        }
        if impl_proc.state == ProcessRunState::Failed {
            effects.push(Effect::SwitchToImplBranch);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                causes: vec![],
                pending_run_id: None,
            });
            return State::ImplementationRunning;
        }
        if impl_proc.state == ProcessRunState::Stale {
            effects.push(Effect::SwitchToImplBranch);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                causes: vec![],
                pending_run_id: None,
            });
            return State::ImplementationRunning;
        }
        if impl_proc.state == ProcessRunState::Succeeded {
            if let Some(impl_pr) = &snap.impl_pr {
                if impl_pr.state == PrState::Merged {
                    effects.push(Effect::PostCompletedComment);
                    effects.push(Effect::CleanupWorktree);
                    effects.push(Effect::CloseIssue);
                    metadata_updates.ci_fix_count = Some(0);
                    return State::Completed;
                }
                if impl_pr.state == PrState::Closed {
                    metadata_updates.ci_fix_count = Some(0);
                    effects.push(Effect::SwitchToImplBranch);
                    effects.push(Effect::SpawnProcess {
                        type_: ProcessRunType::Impl,
                        causes: vec![],
                        pending_run_id: None,
                    });
                    return State::ImplementationRunning;
                }
                if impl_pr.state == PrState::Open {
                    return State::ImplementationReviewWaiting;
                }
            }
            // Succeeded but PR is None (deleted/404): re-spawn
            effects.push(Effect::SwitchToImplBranch);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                causes: vec![],
                pending_run_id: None,
            });
            return State::ImplementationRunning;
        }
        State::ImplementationRunning
    } else {
        // No impl process: spawn (with SwitchToImplBranch first)
        effects.push(Effect::SwitchToImplBranch);
        effects.push(Effect::SpawnProcess {
            type_: ProcessRunType::Impl,
            causes: vec![],
            pending_run_id: None,
        });
        State::ImplementationRunning
    }
}

fn decide_implementation_review_waiting(
    prev: &Issue,
    snap: &WorldSnapshot,
    cfg: &Config,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> State {
    if let Some(s) = try_complete_from_impl_merged(snap, effects, metadata_updates) {
        return s;
    }
    if issue_is_closed(snap) {
        return go_cancelled_issue_closed(effects);
    }

    let impl_pr = match &snap.impl_pr {
        Some(pr) => pr,
        None => {
            emit_spawn_impl(snap, effects);
            return State::ImplementationRunning;
        }
    };

    use crate::domain::world_snapshot::CiStatus;

    if impl_pr.state == PrState::Merged {
        effects.push(Effect::PostCompletedComment);
        effects.push(Effect::CleanupWorktree);
        effects.push(Effect::CloseIssue);
        metadata_updates.ci_fix_count = Some(0);
        return State::Completed;
    }
    if impl_pr.state == PrState::Closed {
        metadata_updates.ci_fix_count = Some(0);
        emit_spawn_impl(snap, effects);
        return State::ImplementationRunning;
    }
    if impl_pr.has_review_comments {
        metadata_updates.ci_fix_count = Some(0);
        emit_spawn_impl_fix(snap, impl_pr, effects);
        return State::ImplementationFixing;
    }
    if impl_pr.ci_status == CiStatus::Failure {
        if snap.ci_fix_exhausted {
            if prev.ci_fix_count == cfg.max_ci_fix_cycles {
                effects.push(Effect::PostCiFixLimitComment);
                metadata_updates.ci_fix_count = Some(prev.ci_fix_count + 1);
            }
            return State::ImplementationReviewWaiting;
        }
        metadata_updates.ci_fix_count = Some(prev.ci_fix_count + 1);
        emit_spawn_impl_fix(snap, impl_pr, effects);
        return State::ImplementationFixing;
    }
    if impl_pr.has_conflict {
        if snap.ci_fix_exhausted {
            if prev.ci_fix_count == cfg.max_ci_fix_cycles {
                effects.push(Effect::PostCiFixLimitComment);
                metadata_updates.ci_fix_count = Some(prev.ci_fix_count + 1);
            }
            return State::ImplementationReviewWaiting;
        }
        metadata_updates.ci_fix_count = Some(prev.ci_fix_count + 1);
        emit_spawn_impl_fix(snap, impl_pr, effects);
        return State::ImplementationFixing;
    }

    State::ImplementationReviewWaiting
}

fn decide_implementation_fixing(
    _prev: &Issue,
    snap: &WorldSnapshot,
    cfg: &Config,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> State {
    if let Some(s) = try_complete_from_impl_merged(snap, effects, metadata_updates) {
        return s;
    }
    if issue_is_closed(snap) {
        return go_cancelled_issue_closed(effects);
    }

    let impl_pr = match &snap.impl_pr {
        Some(pr) => pr,
        None => {
            emit_spawn_impl(snap, effects);
            return State::ImplementationRunning;
        }
    };

    if impl_pr.state == PrState::Merged {
        effects.push(Effect::PostCompletedComment);
        effects.push(Effect::CleanupWorktree);
        effects.push(Effect::CloseIssue);
        metadata_updates.ci_fix_count = Some(0);
        return State::Completed;
    }
    if impl_pr.state == PrState::Closed {
        metadata_updates.ci_fix_count = Some(0);
        emit_spawn_impl(snap, effects);
        return State::ImplementationRunning;
    }

    if let Some(impl_fix) = &snap.processes.impl_fix {
        if impl_fix.consecutive_failures >= cfg.max_retries {
            return go_cancelled_retry_exhausted(effects, ProcessRunType::ImplFix, impl_fix.consecutive_failures);
        }
        if impl_fix.state == ProcessRunState::Pending {
            let causes = derive_fixing_causes(impl_pr);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::ImplFix,
                causes,
                pending_run_id: Some(impl_fix.run_id),
            });
            return State::ImplementationFixing;
        }
        if impl_fix.state == ProcessRunState::Failed {
            let causes = derive_fixing_causes(impl_pr);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::ImplFix,
                causes,
                pending_run_id: None,
            });
            return State::ImplementationFixing;
        }
        if impl_fix.state == ProcessRunState::Stale {
            let causes = derive_fixing_causes(impl_pr);
            effects.push(Effect::SpawnProcess {
                type_: ProcessRunType::ImplFix,
                causes,
                pending_run_id: None,
            });
            return State::ImplementationFixing;
        }
        if impl_fix.state == ProcessRunState::Succeeded {
            return State::ImplementationReviewWaiting;
        }
        State::ImplementationFixing
    } else {
        let causes = derive_fixing_causes(impl_pr);
        effects.push(Effect::SpawnProcess {
            type_: ProcessRunType::ImplFix,
            causes,
            pending_run_id: None,
        });
        State::ImplementationFixing
    }
}

fn decide_completed(
    prev: &Issue,
    _snap: &WorldSnapshot,
    effects: &mut Vec<Effect>,
    _metadata_updates: &mut MetadataUpdates,
) -> State {
    // Persistent effects: CleanupWorktree if worktree exists, CloseIssue if !close_finished
    if prev.worktree_path.is_some() {
        effects.push(Effect::CleanupWorktree);
    }
    if !prev.close_finished {
        effects.push(Effect::CloseIssue);
    }
    State::Completed
}

fn decide_cancelled(
    prev: &Issue,
    snap: &WorldSnapshot,
    effects: &mut Vec<Effect>,
    metadata_updates: &mut MetadataUpdates,
) -> State {
    // Persistent: CloseIssue if !close_finished
    if !prev.close_finished {
        effects.push(Effect::CloseIssue);
        return State::Cancelled;
    }
    // close_finished=true: check whether github_issue is Open
    if matches!(snap.github_issue, GithubIssueSnapshot::Open { .. }) {
        // Cancelled → Idle
        metadata_updates.consecutive_failures_epoch = Some(Some(Utc::now()));
        metadata_updates.close_finished = Some(false);
        return State::Idle;
    }
    // close_finished=true, github_issue is Closed: stay, no effects
    State::Cancelled
}

// ===== TESTS =====

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::world_snapshot::{
        CiStatus, GithubIssueSnapshot, ProcessSnapshot, WorldSnapshot, fixtures,
    };

    fn cfg() -> Config {
        Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string())
    }

    fn make_issue(state: State) -> Issue {
        let mut i = Issue::new(1, "test".to_string());
        i.state = state;
        i
    }

    fn with_worktree(mut issue: Issue) -> Issue {
        issue.worktree_path = Some("/tmp/wt".to_string());
        issue
    }

    fn snap_with_ready_label(trusted: bool) -> WorldSnapshot {
        let mut snap = fixtures::idle();
        snap.github_issue = GithubIssueSnapshot::Open {
            has_ready_label: true,
            ready_label_trusted: trusted,
            weight: None,
        };
        snap
    }

    fn snap_closed() -> WorldSnapshot {
        let mut snap = fixtures::idle();
        snap.github_issue = GithubIssueSnapshot::Closed;
        snap
    }

    fn snap_with_init(state: ProcessRunState, consecutive_failures: u32) -> WorldSnapshot {
        let mut snap = fixtures::idle();
        snap.processes.init = Some(ProcessSnapshot {
            state,
            index: 0,
            run_id: 0,
            consecutive_failures,
        });
        snap
    }

    fn snap_with_design(state: ProcessRunState, consecutive_failures: u32) -> WorldSnapshot {
        let mut snap = fixtures::idle();
        snap.processes.design = Some(ProcessSnapshot {
            state,
            index: 0,
            run_id: 0,
            consecutive_failures,
        });
        snap
    }

    fn snap_with_design_fix(state: ProcessRunState, consecutive_failures: u32) -> WorldSnapshot {
        let mut snap = fixtures::idle();
        snap.design_pr = Some(fixtures::open_pr());
        snap.processes.design_fix = Some(ProcessSnapshot {
            state,
            index: 0,
            run_id: 0,
            consecutive_failures,
        });
        snap
    }

    fn snap_with_impl(state: ProcessRunState, consecutive_failures: u32) -> WorldSnapshot {
        let mut snap = fixtures::idle();
        snap.processes.impl_ = Some(ProcessSnapshot {
            state,
            index: 0,
            run_id: 0,
            consecutive_failures,
        });
        snap
    }

    fn snap_with_impl_fix(state: ProcessRunState, consecutive_failures: u32) -> WorldSnapshot {
        let mut snap = fixtures::idle();
        snap.impl_pr = Some(fixtures::open_pr());
        snap.processes.impl_fix = Some(ProcessSnapshot {
            state,
            index: 0,
            run_id: 0,
            consecutive_failures,
        });
        snap
    }

    // === Idle ===

    /// T-1.D.idle.1
    #[test]
    fn idle_1_ready_label_trusted_goes_to_initialize_running() {
        let issue = make_issue(State::Idle);
        let snap = snap_with_ready_label(true);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::InitializeRunning);
        assert!(d.effects.contains(&Effect::SpawnInit));
    }

    /// T-1.D.idle.2
    #[test]
    fn idle_2_ready_label_untrusted_stays_idle_with_reject() {
        let issue = make_issue(State::Idle);
        let snap = snap_with_ready_label(false);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Idle);
        assert!(d.effects.contains(&Effect::RejectUntrustedReadyIssue));
    }

    /// T-1.D.idle.3
    #[test]
    fn idle_3_no_ready_label_stays_idle_no_effects() {
        let issue = make_issue(State::Idle);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Idle);
        assert!(d.effects.is_empty());
    }

    // === InitializeRunning ===

    /// T-1.D.init.1
    #[test]
    fn init_1_issue_closed_goes_cancelled() {
        let issue = make_issue(State::InitializeRunning);
        let snap = snap_closed();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.contains(&Effect::PostCancelComment));
    }

    /// T-1.D.init.2
    #[test]
    fn init_2_consecutive_failures_at_max_goes_cancelled() {
        let issue = make_issue(State::InitializeRunning);
        let max = cfg().max_retries;
        let snap = snap_with_init(ProcessRunState::Failed, max);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::PostRetryExhaustedComment {
                process_type: ProcessRunType::Init,
                consecutive_failures,
            } if *consecutive_failures == max
        )));
    }

    /// T-1.D.init.3
    #[test]
    fn init_3_init_failed_under_cap_stays_with_spawn_init() {
        let issue = make_issue(State::InitializeRunning);
        let snap = snap_with_init(ProcessRunState::Failed, 1);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::InitializeRunning);
        assert!(d.effects.contains(&Effect::SpawnInit));
    }

    /// T-1.D.init.4
    #[test]
    fn init_4_succeeded_impl_pr_merged_goes_completed() {
        let issue = make_issue(State::InitializeRunning);
        let mut snap = snap_with_init(ProcessRunState::Succeeded, 0);
        snap.impl_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Completed);
        assert!(d.effects.contains(&Effect::PostCompletedComment));
        assert!(d.effects.contains(&Effect::CleanupWorktree));
        assert!(d.effects.contains(&Effect::CloseIssue));
    }

    /// T-1.D.init.5
    #[test]
    fn init_5_succeeded_impl_pr_open_goes_impl_review_waiting() {
        let issue = make_issue(State::InitializeRunning);
        let mut snap = snap_with_init(ProcessRunState::Succeeded, 0);
        snap.impl_pr = Some(fixtures::open_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationReviewWaiting);
    }

    /// T-1.D.init.6
    #[test]
    fn init_6_succeeded_design_pr_merged_goes_impl_running() {
        let issue = make_issue(State::InitializeRunning);
        let mut snap = snap_with_init(ProcessRunState::Succeeded, 0);
        snap.design_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert_eq!(d.metadata_updates.ci_fix_count, Some(0));
    }

    /// T-1.D.init.7
    #[test]
    fn init_7_succeeded_design_pr_open_goes_design_review_waiting() {
        let issue = make_issue(State::InitializeRunning);
        let mut snap = snap_with_init(ProcessRunState::Succeeded, 0);
        snap.design_pr = Some(fixtures::open_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignReviewWaiting);
    }

    /// T-1.D.init.8
    #[test]
    fn init_8_succeeded_no_pr_goes_design_running() {
        let issue = make_issue(State::InitializeRunning);
        let snap = snap_with_init(ProcessRunState::Succeeded, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
    }

    /// T-1.D.init.9
    #[test]
    fn init_9_no_init_process_spawns_init() {
        let issue = make_issue(State::InitializeRunning);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::InitializeRunning);
        assert!(d.effects.contains(&Effect::SpawnInit));
    }

    /// T-1.D.init.10
    #[test]
    fn init_10_closed_state_beats_all_others() {
        // Even with init.succeeded, if issue is closed → Cancelled
        let issue = make_issue(State::InitializeRunning);
        let mut snap = snap_with_init(ProcessRunState::Succeeded, 0);
        snap.impl_pr = Some(fixtures::merged_pr());
        snap.github_issue = GithubIssueSnapshot::Closed;
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.contains(&Effect::PostCancelComment));
    }

    // === DesignRunning ===

    /// T-1.D.dr.1
    #[test]
    fn dr_1_issue_closed_goes_cancelled() {
        let issue = make_issue(State::DesignRunning);
        let snap = snap_closed();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.contains(&Effect::PostCancelComment));
    }

    /// T-1.D.dr.2
    #[test]
    fn dr_2_consecutive_failures_at_max_goes_cancelled() {
        let issue = make_issue(State::DesignRunning);
        let max = cfg().max_retries;
        let snap = snap_with_design(ProcessRunState::Failed, max);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::PostRetryExhaustedComment {
                process_type: ProcessRunType::Design,
                consecutive_failures,
            } if *consecutive_failures == max
        )));
    }

    /// T-1.D.dr.3
    #[test]
    fn dr_3_design_failed_under_cap_stays_with_spawn() {
        let issue = make_issue(State::DesignRunning);
        let snap = snap_with_design(ProcessRunState::Failed, 1);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                ..
            }
        )));
    }

    /// T-1.D.dr.4
    #[test]
    fn dr_4_design_stale_stays_with_spawn() {
        let issue = make_issue(State::DesignRunning);
        let snap = snap_with_design(ProcessRunState::Stale, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                ..
            }
        )));
    }

    /// T-1.D.dr.4b: pending stays in DesignRunning with SpawnProcess(pending_run_id=Some)
    #[test]
    fn dr_4b_design_pending_stays_with_spawn() {
        let issue = make_issue(State::DesignRunning);
        let snap = snap_with_design(ProcessRunState::Pending, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                pending_run_id: Some(_),
                ..
            }
        )));
    }

    /// T-1.D.dr.5
    #[test]
    fn dr_5_design_succeeded_pr_merged_goes_impl_running() {
        let issue = make_issue(State::DesignRunning);
        let mut snap = snap_with_design(ProcessRunState::Succeeded, 0);
        snap.design_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert_eq!(d.metadata_updates.ci_fix_count, Some(0));
    }

    /// T-1.D.dr.6
    #[test]
    fn dr_6_design_succeeded_pr_closed_stays_with_spawn() {
        let issue = make_issue(State::DesignRunning);
        let mut snap = snap_with_design(ProcessRunState::Succeeded, 0);
        snap.design_pr = Some(fixtures::closed_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
        assert_eq!(d.metadata_updates.ci_fix_count, Some(0));
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                ..
            }
        )));
    }

    /// T-1.D.dr.7
    #[test]
    fn dr_7_design_succeeded_pr_open_goes_design_review_waiting() {
        let issue = make_issue(State::DesignRunning);
        let mut snap = snap_with_design(ProcessRunState::Succeeded, 0);
        snap.design_pr = Some(fixtures::open_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignReviewWaiting);
    }

    /// T-1.D.dr.8
    #[test]
    fn dr_8_no_process_spawns_design() {
        let issue = make_issue(State::DesignRunning);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                ..
            }
        )));
    }

    // === DesignReviewWaiting ===

    /// T-1.D.drw.1
    #[test]
    fn drw_1_issue_closed_goes_cancelled() {
        let issue = make_issue(State::DesignReviewWaiting);
        let snap = snap_closed();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
    }

    /// T-1.D.drw.2
    #[test]
    fn drw_2_design_pr_merged_goes_impl_running() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        snap.design_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert_eq!(d.metadata_updates.ci_fix_count, Some(0));
    }

    /// T-1.D.drw.3
    #[test]
    fn drw_3_design_pr_closed_goes_design_running() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        snap.design_pr = Some(fixtures::closed_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
        assert_eq!(d.metadata_updates.ci_fix_count, Some(0));
    }

    /// T-1.D.drw.4
    #[test]
    fn drw_4_has_review_comments_goes_design_fixing_with_reset() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_review_comments = true;
        snap.design_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert_eq!(d.metadata_updates.ci_fix_count, Some(0));
    }

    /// T-1.D.drw.5
    #[test]
    fn drw_5_review_comments_plus_ci_failure_goes_fixing_with_reset() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_review_comments = true;
        pr.ci_status = CiStatus::Failure;
        snap.design_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        // review wins reset
        assert_eq!(d.metadata_updates.ci_fix_count, Some(0));
    }

    /// T-1.D.drw.6
    #[test]
    fn drw_6_ci_failure_exhausted_stays_with_comment() {
        let cfg = cfg();
        let mut issue = make_issue(State::DesignReviewWaiting);
        issue.ci_fix_count = cfg.max_ci_fix_cycles; // at max
        let mut snap = fixtures::idle();
        snap.ci_fix_exhausted = true;
        let mut pr = fixtures::open_pr();
        pr.ci_status = CiStatus::Failure;
        snap.design_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg);
        assert_eq!(d.next_state, State::DesignReviewWaiting);
        assert!(d.effects.contains(&Effect::PostCiFixLimitComment));
        assert_eq!(
            d.metadata_updates.ci_fix_count,
            Some(cfg.max_ci_fix_cycles + 1)
        );
    }

    /// T-1.D.drw.7
    #[test]
    fn drw_7_ci_failure_under_cap_goes_design_fixing() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.ci_status = CiStatus::Failure;
        snap.design_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert_eq!(d.metadata_updates.ci_fix_count, Some(1));
    }

    /// T-1.D.drw.8
    #[test]
    fn drw_8_conflict_exhausted_stays_with_comment() {
        let cfg = cfg();
        let mut issue = make_issue(State::DesignReviewWaiting);
        issue.ci_fix_count = cfg.max_ci_fix_cycles;
        let mut snap = fixtures::idle();
        snap.ci_fix_exhausted = true;
        let mut pr = fixtures::open_pr();
        pr.has_conflict = true;
        snap.design_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg);
        assert_eq!(d.next_state, State::DesignReviewWaiting);
        assert!(d.effects.contains(&Effect::PostCiFixLimitComment));
    }

    /// T-1.D.drw.9
    #[test]
    fn drw_9_conflict_under_cap_goes_design_fixing() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_conflict = true;
        snap.design_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert_eq!(d.metadata_updates.ci_fix_count, Some(1));
    }

    /// T-1.D.drw.10
    #[test]
    fn drw_10_no_design_pr_goes_design_running() {
        let issue = make_issue(State::DesignReviewWaiting);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
    }

    /// T-1.D.drw.11
    #[test]
    fn drw_11_priority_merge_beats_close_beats_review_beats_ci_beats_conflict() {
        let cfg = cfg();
        // merge beats all
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::merged_pr();
        pr.has_review_comments = true;
        pr.ci_status = CiStatus::Failure;
        pr.has_conflict = true;
        snap.design_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg);
        assert_eq!(d.next_state, State::ImplementationRunning);
    }

    /// T-1.D.drw.12
    #[test]
    fn drw_12_no_comment_when_ci_fix_count_already_max_plus_1() {
        let cfg = cfg();
        let mut issue = make_issue(State::DesignReviewWaiting);
        issue.ci_fix_count = cfg.max_ci_fix_cycles + 1; // already max+1
        let mut snap = fixtures::idle();
        snap.ci_fix_exhausted = true; // still exhausted
        let mut pr = fixtures::open_pr();
        pr.ci_status = CiStatus::Failure;
        snap.design_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg);
        assert_eq!(d.next_state, State::DesignReviewWaiting);
        // No PostCiFixLimitComment: trigger is == max, not >= max
        assert!(!d.effects.contains(&Effect::PostCiFixLimitComment));
    }

    // === DesignFixing ===

    /// T-1.D.df.1
    #[test]
    fn df_1_issue_closed_goes_cancelled() {
        let issue = make_issue(State::DesignFixing);
        let snap = snap_closed();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
    }

    /// T-1.D.df.2
    #[test]
    fn df_2_design_pr_merged_goes_impl_running() {
        let issue = make_issue(State::DesignFixing);
        let mut snap = fixtures::idle();
        snap.design_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
    }

    /// T-1.D.df.3
    #[test]
    fn df_3_design_pr_closed_goes_design_running() {
        let issue = make_issue(State::DesignFixing);
        let mut snap = fixtures::idle();
        snap.design_pr = Some(fixtures::closed_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
    }

    /// T-1.D.df.4
    #[test]
    fn df_4_no_design_pr_goes_design_running() {
        let issue = make_issue(State::DesignFixing);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
    }

    /// T-1.D.df.5
    #[test]
    fn df_5_design_fix_failures_at_max_goes_cancelled() {
        let issue = make_issue(State::DesignFixing);
        let max = cfg().max_retries;
        let snap = snap_with_design_fix(ProcessRunState::Failed, max);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::PostRetryExhaustedComment {
                process_type: ProcessRunType::DesignFix,
                consecutive_failures,
            } if *consecutive_failures == max
        )));
    }

    /// T-1.D.df.6
    #[test]
    fn df_6_design_fix_failed_under_cap_stays_with_spawn() {
        let issue = make_issue(State::DesignFixing);
        let snap = snap_with_design_fix(ProcessRunState::Failed, 1);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                ..
            }
        )));
    }

    /// T-1.D.df.7
    #[test]
    fn df_7_design_fix_stale_stays_with_spawn() {
        let issue = make_issue(State::DesignFixing);
        let snap = snap_with_design_fix(ProcessRunState::Stale, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                ..
            }
        )));
    }

    /// T-1.D.df.7b: pending stays in DesignFixing with SpawnProcess(pending_run_id=Some)
    #[test]
    fn df_7b_design_fix_pending_stays_with_spawn() {
        let issue = make_issue(State::DesignFixing);
        let snap = snap_with_design_fix(ProcessRunState::Pending, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                pending_run_id: Some(_),
                ..
            }
        )));
    }

    /// T-1.D.df.8
    #[test]
    fn df_8_design_fix_succeeded_goes_design_review_waiting() {
        let issue = make_issue(State::DesignFixing);
        let snap = snap_with_design_fix(ProcessRunState::Succeeded, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignReviewWaiting);
    }

    /// T-1.D.df.9
    #[test]
    fn df_9_no_process_spawns_design_fix() {
        let issue = make_issue(State::DesignFixing);
        let mut snap = fixtures::idle();
        snap.design_pr = Some(fixtures::open_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                ..
            }
        )));
    }

    /// T-1.D.df.10
    #[test]
    fn df_10_causes_rederived_from_world_snapshot() {
        let issue = make_issue(State::DesignFixing);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.ci_status = CiStatus::Failure;
        snap.design_pr = Some(pr);
        // No design_fix process
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        if let Some(Effect::SpawnProcess { causes, .. }) = d.effects.first() {
            assert!(causes.contains(&FixingProblemKind::CiFailure));
        }
    }

    // === ImplementationRunning (mirror of DesignRunning) ===

    /// T-1.D.ir.1
    #[test]
    fn ir_1_issue_closed_goes_cancelled() {
        let issue = make_issue(State::ImplementationRunning);
        let snap = snap_closed();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
    }

    /// T-1.D.ir.2
    #[test]
    fn ir_2_consecutive_failures_at_max_goes_cancelled() {
        let issue = make_issue(State::ImplementationRunning);
        let max = cfg().max_retries;
        let snap = snap_with_impl(ProcessRunState::Failed, max);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::PostRetryExhaustedComment {
                process_type: ProcessRunType::Impl,
                consecutive_failures,
            } if *consecutive_failures == max
        )));
    }

    /// T-1.D.ir.3
    #[test]
    fn ir_3_impl_failed_stays_with_switch_and_spawn() {
        let issue = make_issue(State::ImplementationRunning);
        let snap = snap_with_impl(ProcessRunState::Failed, 1);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert!(d.effects.contains(&Effect::SwitchToImplBranch));
    }

    /// T-1.D.ir.4
    #[test]
    fn ir_4_impl_stale_stays_with_switch_and_spawn() {
        let issue = make_issue(State::ImplementationRunning);
        let snap = snap_with_impl(ProcessRunState::Stale, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert!(d.effects.contains(&Effect::SwitchToImplBranch));
    }

    /// T-1.D.ir.4b: pending stays in ImplementationRunning with SpawnProcess(pending_run_id=Some)
    #[test]
    fn ir_4b_impl_pending_stays_with_spawn() {
        let issue = make_issue(State::ImplementationRunning);
        let snap = snap_with_impl(ProcessRunState::Pending, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        // Pending skips SwitchToImplBranch (already done on first attempt)
        assert!(
            !d.effects
                .iter()
                .any(|e| matches!(e, Effect::SwitchToImplBranch))
        );
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                pending_run_id: Some(_),
                ..
            }
        )));
    }

    /// T-1.D.ir.5 (impl.succeeded + impl_pr.merged → Completed)
    #[test]
    fn ir_5_impl_succeeded_pr_merged_goes_completed() {
        let issue = make_issue(State::ImplementationRunning);
        let mut snap = snap_with_impl(ProcessRunState::Succeeded, 0);
        snap.impl_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Completed);
        assert!(d.effects.contains(&Effect::PostCompletedComment));
    }

    /// T-1.D.ir.6 (impl.succeeded + impl_pr.closed → ImplementationRunning)
    #[test]
    fn ir_6_impl_succeeded_pr_closed_stays_impl_running() {
        let issue = make_issue(State::ImplementationRunning);
        let mut snap = snap_with_impl(ProcessRunState::Succeeded, 0);
        snap.impl_pr = Some(fixtures::closed_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
    }

    /// T-1.D.ir.7 (impl.succeeded + impl_pr.open → ImplementationReviewWaiting)
    #[test]
    fn ir_7_impl_succeeded_pr_open_goes_impl_review_waiting() {
        let issue = make_issue(State::ImplementationRunning);
        let mut snap = snap_with_impl(ProcessRunState::Succeeded, 0);
        snap.impl_pr = Some(fixtures::open_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationReviewWaiting);
    }

    /// T-1.D.ir.8 (no process → spawn with SwitchToImplBranch)
    #[test]
    fn ir_8_no_process_spawns_with_switch() {
        let issue = make_issue(State::ImplementationRunning);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert!(d.effects.contains(&Effect::SwitchToImplBranch));
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                ..
            }
        )));
    }

    /// T-1.D.ir.9: SwitchToImplBranch ordered before SpawnProcess
    #[test]
    fn ir_9_switch_ordered_before_spawn_process() {
        let issue = make_issue(State::ImplementationRunning);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        let switch_pos = d
            .effects
            .iter()
            .position(|e| e == &Effect::SwitchToImplBranch);
        let spawn_pos = d
            .effects
            .iter()
            .position(|e| matches!(e, Effect::SpawnProcess { .. }));
        assert!(switch_pos.is_some() && spawn_pos.is_some());
        assert!(switch_pos.unwrap() < spawn_pos.unwrap());
    }

    // === ImplementationReviewWaiting ===

    /// T-1.D.irw.merge
    #[test]
    fn irw_merge_impl_pr_merged_goes_completed() {
        let issue = make_issue(State::ImplementationReviewWaiting);
        let mut snap = fixtures::idle();
        snap.impl_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Completed);
        assert!(d.effects.contains(&Effect::PostCompletedComment));
        assert!(d.effects.contains(&Effect::CleanupWorktree));
        assert!(d.effects.contains(&Effect::CloseIssue));
        assert_eq!(d.metadata_updates.ci_fix_count, Some(0));
    }

    // Checking remaining mirror rows
    #[test]
    fn irw_closed_goes_cancelled() {
        let issue = make_issue(State::ImplementationReviewWaiting);
        let snap = snap_closed();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
    }

    #[test]
    fn irw_closed_pr_goes_impl_running() {
        let issue = make_issue(State::ImplementationReviewWaiting);
        let mut snap = fixtures::idle();
        snap.impl_pr = Some(fixtures::closed_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
    }

    #[test]
    fn irw_review_comments_goes_impl_fixing() {
        let issue = make_issue(State::ImplementationReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_review_comments = true;
        snap.impl_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationFixing);
        assert_eq!(d.metadata_updates.ci_fix_count, Some(0));
    }

    #[test]
    fn irw_no_impl_pr_goes_impl_running() {
        let issue = make_issue(State::ImplementationReviewWaiting);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
    }

    // === ImplementationFixing ===

    #[test]
    fn if_1_issue_closed_goes_cancelled() {
        let issue = make_issue(State::ImplementationFixing);
        let snap = snap_closed();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
    }

    #[test]
    fn if_2_impl_pr_merged_goes_completed() {
        let issue = make_issue(State::ImplementationFixing);
        let mut snap = fixtures::idle();
        snap.impl_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Completed);
    }

    #[test]
    fn if_3_impl_pr_closed_goes_impl_running() {
        let issue = make_issue(State::ImplementationFixing);
        let mut snap = fixtures::idle();
        snap.impl_pr = Some(fixtures::closed_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
    }

    #[test]
    fn if_4_no_impl_pr_goes_impl_running() {
        let issue = make_issue(State::ImplementationFixing);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
    }

    #[test]
    fn if_5_impl_fix_failures_at_max_goes_cancelled() {
        let issue = make_issue(State::ImplementationFixing);
        let max = cfg().max_retries;
        let snap = snap_with_impl_fix(ProcessRunState::Failed, max);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::PostRetryExhaustedComment {
                process_type: ProcessRunType::ImplFix,
                consecutive_failures,
            } if *consecutive_failures == max
        )));
    }

    #[test]
    fn if_6_impl_fix_failed_stays_with_spawn() {
        let issue = make_issue(State::ImplementationFixing);
        let snap = snap_with_impl_fix(ProcessRunState::Failed, 1);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::ImplFix,
                ..
            }
        )));
    }

    #[test]
    fn if_7_impl_fix_stale_stays_with_spawn() {
        let issue = make_issue(State::ImplementationFixing);
        let snap = snap_with_impl_fix(ProcessRunState::Stale, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationFixing);
    }

    #[test]
    fn if_7b_impl_fix_pending_stays_with_spawn() {
        let issue = make_issue(State::ImplementationFixing);
        let snap = snap_with_impl_fix(ProcessRunState::Pending, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::ImplFix,
                pending_run_id: Some(_),
                ..
            }
        )));
    }

    #[test]
    fn if_8_impl_fix_succeeded_goes_impl_review_waiting() {
        let issue = make_issue(State::ImplementationFixing);
        let snap = snap_with_impl_fix(ProcessRunState::Succeeded, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationReviewWaiting);
    }

    #[test]
    fn if_9_no_process_spawns() {
        let issue = make_issue(State::ImplementationFixing);
        let mut snap = fixtures::idle();
        snap.impl_pr = Some(fixtures::open_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::ImplFix,
                ..
            }
        )));
    }

    #[test]
    fn if_10_causes_rederived_from_snapshot() {
        let issue = make_issue(State::ImplementationFixing);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_conflict = true;
        snap.impl_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg());
        if let Some(Effect::SpawnProcess { causes, .. }) = d.effects.first() {
            assert!(causes.contains(&FixingProblemKind::Conflict));
        }
    }

    // === Completed ===

    /// T-1.D.c.1
    #[test]
    fn c_1_stays_completed() {
        let issue = make_issue(State::Completed);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Completed);
    }

    /// T-1.D.c.2
    #[test]
    fn c_2_worktree_path_not_null_emits_cleanup_worktree() {
        let issue = with_worktree(make_issue(State::Completed));
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert!(d.effects.contains(&Effect::CleanupWorktree));
    }

    /// T-1.D.c.3
    #[test]
    fn c_3_not_close_finished_emits_close_issue() {
        let issue = make_issue(State::Completed);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert!(d.effects.contains(&Effect::CloseIssue));
    }

    /// T-1.D.c.4
    #[test]
    fn c_4_worktree_null_and_close_finished_no_effects() {
        let mut issue = make_issue(State::Completed);
        issue.close_finished = true;
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert!(d.effects.is_empty());
    }

    // === Cancelled ===

    /// T-1.D.x.1
    #[test]
    fn x_1_close_finished_issue_open_goes_idle() {
        let mut issue = make_issue(State::Cancelled);
        issue.close_finished = true;
        let snap = fixtures::idle(); // github_issue is Open
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Idle);
        assert!(d.metadata_updates.consecutive_failures_epoch.is_some());
        assert_eq!(d.metadata_updates.close_finished, Some(false));
    }

    /// T-1.D.x.2
    #[test]
    fn x_2_not_close_finished_stays_cancelled_with_close_issue() {
        let issue = make_issue(State::Cancelled);
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.contains(&Effect::CloseIssue));
    }

    /// T-1.D.x.3
    #[test]
    fn x_3_close_finished_github_issue_closed_stays_cancelled_no_effects() {
        let mut issue = make_issue(State::Cancelled);
        issue.close_finished = true;
        let snap = snap_closed();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Cancelled);
        assert!(d.effects.is_empty());
    }

    /// T-1.D.x.4
    #[test]
    fn x_4_cancelled_to_idle_does_not_reset_ci_fix_count() {
        let mut issue = make_issue(State::Cancelled);
        issue.close_finished = true;
        issue.ci_fix_count = 3;
        let snap = fixtures::idle();
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::Idle);
        // ci_fix_count should NOT be reset
        assert!(d.metadata_updates.ci_fix_count.is_none());
    }

    // === Cross-cutting ===

    /// T-1.D.X.1
    #[test]
    fn cross_1_weight_propagated_when_changed() {
        use crate::domain::task_weight::TaskWeight;
        let mut issue = make_issue(State::Idle);
        issue.weight = TaskWeight::Light;
        let mut snap = fixtures::idle();
        snap.github_issue = GithubIssueSnapshot::Open {
            has_ready_label: false,
            ready_label_trusted: false,
            weight: Some(TaskWeight::Heavy),
        };
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.metadata_updates.weight, Some(TaskWeight::Heavy));
    }

    /// T-1.D.X.2
    #[test]
    fn cross_2_effects_in_priority_order() {
        let issue = make_issue(State::Completed);
        let mut issue2 = issue.clone();
        issue2.worktree_path = Some("/tmp/wt".to_string());
        let snap = fixtures::idle();
        let d = decide(&issue2, &snap, &cfg());
        // CleanupWorktree (6) before CloseIssue (7)
        let cleanup_pos = d.effects.iter().position(|e| e == &Effect::CleanupWorktree);
        let close_pos = d.effects.iter().position(|e| e == &Effect::CloseIssue);
        if let (Some(cp), Some(cl)) = (cleanup_pos, close_pos) {
            assert!(cp < cl);
        }
    }

    /// T-1.D.X.3: decide performs no I/O (compile-time: it takes only borrowed inputs)
    #[test]
    fn cross_3_decide_performs_no_io() {
        let issue = make_issue(State::Idle);
        let snap = fixtures::idle();
        // If this compiles and runs, it's pure
        let _d = decide(&issue, &snap, &cfg());
    }

    /// T-1.D.X.4
    #[test]
    fn cross_4_post_ci_fix_limit_and_count_produced_together() {
        let cfg = cfg();
        let mut issue = make_issue(State::DesignReviewWaiting);
        issue.ci_fix_count = cfg.max_ci_fix_cycles;
        let mut snap = fixtures::idle();
        snap.ci_fix_exhausted = true;
        let mut pr = fixtures::open_pr();
        pr.ci_status = CiStatus::Failure;
        snap.design_pr = Some(pr);
        let d = decide(&issue, &snap, &cfg);
        assert!(d.effects.contains(&Effect::PostCiFixLimitComment));
        assert_eq!(
            d.metadata_updates.ci_fix_count,
            Some(cfg.max_ci_fix_cycles + 1)
        );
    }

    // ── Transition-point persistent effect tests ──────────────────────

    /// ReviewWaiting → Fixing with stale Succeeded emits SpawnProcess
    #[test]
    fn drw_to_df_with_stale_succeeded_emits_spawn_process() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_review_comments = true;
        snap.design_pr = Some(pr);
        snap.processes.design_fix = Some(ProcessSnapshot {
            state: ProcessRunState::Succeeded,
            index: 0,
            run_id: 23,
            consecutive_failures: 0,
        });
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                ..
            }
        )));
    }

    /// ImplementationReviewWaiting → ImplementationFixing with stale Succeeded
    #[test]
    fn irw_to_if_with_stale_succeeded_emits_spawn_process() {
        let issue = make_issue(State::ImplementationReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_review_comments = true;
        snap.impl_pr = Some(pr);
        snap.processes.impl_fix = Some(ProcessSnapshot {
            state: ProcessRunState::Succeeded,
            index: 0,
            run_id: 30,
            consecutive_failures: 0,
        });
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::ImplFix,
                ..
            }
        )));
    }

    /// Idle → InitializeRunning emits SpawnInit
    #[test]
    fn idle_to_init_emits_spawn_init() {
        let issue = make_issue(State::Idle);
        let snap = snap_with_ready_label(true);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::InitializeRunning);
        assert!(d.effects.contains(&Effect::SpawnInit));
    }

    /// InitializeRunning → DesignRunning emits SpawnProcess(Design)
    #[test]
    fn init_to_dr_emits_spawn_design() {
        let issue = make_issue(State::InitializeRunning);
        let snap = snap_with_init(ProcessRunState::Succeeded, 0);
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                ..
            }
        )));
    }

    /// InitializeRunning → ImplementationRunning (design merged) emits SwitchToImplBranch + SpawnProcess(Impl)
    #[test]
    fn init_to_ir_emits_spawn_impl() {
        let issue = make_issue(State::InitializeRunning);
        let mut snap = snap_with_init(ProcessRunState::Succeeded, 0);
        snap.design_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert!(d.effects.contains(&Effect::SwitchToImplBranch));
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                ..
            }
        )));
    }

    /// DesignRunning → ImplementationRunning (design merged) emits SwitchToImplBranch + SpawnProcess(Impl)
    #[test]
    fn dr_to_ir_emits_spawn_impl() {
        let issue = make_issue(State::DesignRunning);
        let mut snap = snap_with_design(ProcessRunState::Succeeded, 0);
        snap.design_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert!(d.effects.contains(&Effect::SwitchToImplBranch));
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                ..
            }
        )));
    }

    /// DesignReviewWaiting → DesignRunning (PR None) emits SpawnProcess(Design)
    #[test]
    fn drw_to_dr_no_pr_emits_spawn_design() {
        let issue = make_issue(State::DesignReviewWaiting);
        let snap = fixtures::idle(); // no design_pr
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                ..
            }
        )));
    }

    /// DesignReviewWaiting → ImplementationRunning (merged) emits spawn chain
    #[test]
    fn drw_to_ir_merged_emits_spawn_impl() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        snap.design_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert!(d.effects.contains(&Effect::SwitchToImplBranch));
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                ..
            }
        )));
    }

    /// DesignFixing → DesignRunning (PR closed) emits SpawnProcess(Design)
    #[test]
    fn df_to_dr_pr_closed_emits_spawn_design() {
        let issue = make_issue(State::DesignFixing);
        let mut snap = fixtures::idle();
        snap.design_pr = Some(fixtures::closed_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignRunning);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                ..
            }
        )));
    }

    /// DesignFixing → ImplementationRunning (merged) emits spawn chain
    #[test]
    fn df_to_ir_merged_emits_spawn_impl() {
        let issue = make_issue(State::DesignFixing);
        let mut snap = fixtures::idle();
        snap.design_pr = Some(fixtures::merged_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert!(d.effects.contains(&Effect::SwitchToImplBranch));
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                ..
            }
        )));
    }

    /// ImplementationReviewWaiting → ImplementationRunning (PR closed) emits spawn chain
    #[test]
    fn irw_to_ir_pr_closed_emits_spawn_impl() {
        let issue = make_issue(State::ImplementationReviewWaiting);
        let mut snap = fixtures::idle();
        snap.impl_pr = Some(fixtures::closed_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert!(d.effects.contains(&Effect::SwitchToImplBranch));
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                ..
            }
        )));
    }

    /// ImplementationFixing → ImplementationRunning (PR closed) emits spawn chain
    #[test]
    fn if_to_ir_pr_closed_emits_spawn_impl() {
        let issue = make_issue(State::ImplementationFixing);
        let mut snap = fixtures::idle();
        snap.impl_pr = Some(fixtures::closed_pr());
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::ImplementationRunning);
        assert!(d.effects.contains(&Effect::SwitchToImplBranch));
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::Impl,
                ..
            }
        )));
    }

    /// ReviewWaiting → Fixing with no prior process emits SpawnProcess
    #[test]
    fn drw_to_df_no_prior_process_emits_spawn() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_review_comments = true;
        snap.design_pr = Some(pr);
        // No design_fix process at all
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                pending_run_id: None,
                ..
            }
        )));
    }

    /// ReviewWaiting → Fixing with pending process emits SpawnProcess(pending_run_id=Some)
    #[test]
    fn drw_to_df_pending_process_emits_spawn_with_pending_id() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_review_comments = true;
        snap.design_pr = Some(pr);
        snap.processes.design_fix = Some(ProcessSnapshot {
            state: ProcessRunState::Pending,
            index: 1,
            run_id: 42,
            consecutive_failures: 0,
        });
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert!(d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                pending_run_id: Some(42),
                ..
            }
        )));
    }

    /// ReviewWaiting → Fixing with running process does NOT emit SpawnProcess
    #[test]
    fn drw_to_df_running_process_no_spawn() {
        let issue = make_issue(State::DesignReviewWaiting);
        let mut snap = fixtures::idle();
        let mut pr = fixtures::open_pr();
        pr.has_review_comments = true;
        snap.design_pr = Some(pr);
        snap.processes.design_fix = Some(ProcessSnapshot {
            state: ProcessRunState::Running,
            index: 1,
            run_id: 42,
            consecutive_failures: 0,
        });
        let d = decide(&issue, &snap, &cfg());
        assert_eq!(d.next_state, State::DesignFixing);
        assert!(!d.effects.iter().any(|e| matches!(
            e,
            Effect::SpawnProcess {
                type_: ProcessRunType::DesignFix,
                ..
            }
        )));
    }
}
