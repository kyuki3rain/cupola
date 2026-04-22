use anyhow::Result;

use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::domain::phase::Phase;
use crate::domain::process_run::{ProcessRunState, ProcessRunType};
use crate::domain::state::State;

pub(super) async fn get_pr_number_for_type<P: ProcessRunRepository>(
    process_repo: &P,
    issue_id: i64,
    type_: ProcessRunType,
) -> Result<Option<u64>> {
    let lookup_type = match type_ {
        ProcessRunType::DesignFix => ProcessRunType::Design,
        ProcessRunType::ImplFix => ProcessRunType::Impl,
        _ => type_,
    };
    let latest = process_repo.find_latest(issue_id, lookup_type).await?;
    Ok(latest.and_then(|r| r.pr_number))
}

pub(super) fn state_from_phase(phase: Phase, type_: ProcessRunType) -> State {
    use ProcessRunType::*;

    match (phase, type_) {
        (Phase::Design, Design) => State::DesignRunning,
        (Phase::DesignFix, DesignFix) => State::DesignFixing,
        (Phase::Implementation, Impl) => State::ImplementationRunning,
        (Phase::ImplementationFix, ImplFix) => State::ImplementationFixing,
        _ => State::DesignRunning,
    }
}

pub(super) fn phase_for_type(type_: ProcessRunType) -> Option<Phase> {
    match type_ {
        ProcessRunType::Init => None,
        ProcessRunType::Design => Some(Phase::Design),
        ProcessRunType::DesignFix => Some(Phase::DesignFix),
        ProcessRunType::Impl => Some(Phase::Implementation),
        ProcessRunType::ImplFix => Some(Phase::ImplementationFix),
    }
}

pub(super) async fn find_last_error<P: ProcessRunRepository>(
    process_repo: &P,
    issue_id: i64,
    process_type: ProcessRunType,
) -> Option<String> {
    let runs = process_repo.find_by_issue(issue_id).await.ok()?;
    runs.into_iter()
        .rev()
        .find(|r| r.state == ProcessRunState::Failed && r.type_ == process_type)
        .and_then(|r| r.error_message)
}
