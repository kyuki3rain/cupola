use anyhow::Result;

use crate::domain::issue::Issue;
use crate::domain::state::State;

pub trait IssueRepository: Send + Sync {
    fn find_by_id(
        &self,
        id: i64,
    ) -> impl std::future::Future<Output = Result<Option<Issue>>> + Send;
    fn find_by_issue_number(
        &self,
        issue_number: u64,
    ) -> impl std::future::Future<Output = Result<Option<Issue>>> + Send;
    fn find_active(&self) -> impl std::future::Future<Output = Result<Vec<Issue>>> + Send;
    fn find_needing_process(&self) -> impl std::future::Future<Output = Result<Vec<Issue>>> + Send;
    fn save(&self, issue: &Issue) -> impl std::future::Future<Output = Result<i64>> + Send;
    fn update_state(
        &self,
        id: i64,
        state: State,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn update(&self, issue: &Issue) -> impl std::future::Future<Output = Result<()>> + Send;
    fn reset_for_restart(&self, id: i64) -> impl std::future::Future<Output = Result<()>> + Send;
}
