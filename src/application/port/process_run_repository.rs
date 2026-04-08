use anyhow::Result;

use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};

pub trait ProcessRunRepository: Send + Sync {
    /// Insert a new ProcessRun record and return its assigned id.
    fn save(&self, run: &ProcessRun) -> impl std::future::Future<Output = Result<i64>> + Send;

    /// Update pid for a running process after spawn.
    fn update_pid(
        &self,
        run_id: i64,
        pid: u32,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Transition state to Succeeded and set pr_number / finished_at.
    fn mark_succeeded(
        &self,
        run_id: i64,
        pr_number: Option<u64>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Transition state to Failed and set error_message / finished_at.
    fn mark_failed(
        &self,
        run_id: i64,
        error_message: Option<String>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Mark all Running records for an issue as Stale (used on startup recovery).
    fn mark_stale_for_issue(
        &self,
        issue_id: i64,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Return the most recent ProcessRun for an issue with a given type.
    fn find_latest(
        &self,
        issue_id: i64,
        type_: ProcessRunType,
    ) -> impl std::future::Future<Output = Result<Option<ProcessRun>>> + Send;

    /// Return all ProcessRuns for an issue, ordered by id ascending.
    fn find_by_issue(
        &self,
        issue_id: i64,
    ) -> impl std::future::Future<Output = Result<Vec<ProcessRun>>> + Send;

    /// Count consecutive failures at the tail of the run list for a given type.
    /// Returns 0 if the latest run is not Failed.
    fn count_consecutive_failures(
        &self,
        issue_id: i64,
        type_: ProcessRunType,
    ) -> impl std::future::Future<Output = Result<u32>> + Send;

    /// Return all currently Running records (used on startup recovery).
    fn find_all_running(&self)
    -> impl std::future::Future<Output = Result<Vec<ProcessRun>>> + Send;

    /// Update state directly (e.g. for Stale transition).
    fn update_state(
        &self,
        run_id: i64,
        state: ProcessRunState,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}
