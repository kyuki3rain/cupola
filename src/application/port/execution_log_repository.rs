use anyhow::Result;

use crate::domain::execution_log::ExecutionLog;
use crate::domain::state::State;

pub trait ExecutionLogRepository: Send + Sync {
    fn record_start(
        &self,
        issue_id: i64,
        state: State,
    ) -> impl std::future::Future<Output = Result<i64>> + Send;

    fn record_finish(
        &self,
        log_id: i64,
        exit_code: Option<i32>,
        structured_output: Option<&str>,
        error_message: Option<&str>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn find_by_issue(
        &self,
        issue_id: i64,
    ) -> impl std::future::Future<Output = Result<Vec<ExecutionLog>>> + Send;
}
