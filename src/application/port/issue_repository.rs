use anyhow::Result;

use crate::domain::issue::Issue;
use crate::domain::metadata_update::MetadataUpdates;
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
    fn find_all(&self) -> impl std::future::Future<Output = Result<Vec<Issue>>> + Send;
    /// Active issues + terminal issues whose persistent effects have not
    /// yet converged (worktree still exists or close not finished).
    fn find_observable(&self) -> impl std::future::Future<Output = Result<Vec<Issue>>> + Send;
    fn save(&self, issue: &Issue) -> impl std::future::Future<Output = Result<i64>> + Send;
    fn update_state(
        &self,
        id: i64,
        state: State,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn update(&self, issue: &Issue) -> impl std::future::Future<Output = Result<()>> + Send;
    fn update_state_and_metadata(
        &self,
        issue_id: i64,
        updates: &MetadataUpdates,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn find_by_state(
        &self,
        state: State,
    ) -> impl std::future::Future<Output = Result<Vec<Issue>>> + Send;
}
