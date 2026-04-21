/// Persist phase: write Decide's decisions to DB.
///
/// This phase only writes DB state. No GitHub mutations, no process spawning.
use anyhow::Result;

use crate::application::port::issue_repository::IssueRepository;
use crate::domain::decision::Decision;
use crate::domain::issue::Issue;

/// Persist one decision for one issue.
///
/// Writes `next_state` and `metadata_updates` to the DB.
pub async fn persist_decision<I: IssueRepository>(
    issue_repo: &I,
    issue: &Issue,
    decision: &Decision,
) -> Result<()> {
    // Only update if something changed
    let updates = &decision.metadata_updates;
    let state_changed = decision.next_state != issue.state;

    // If nothing changed at all, skip the DB write
    if !state_changed
        && updates.state.is_none()
        && updates.weight.is_none()
        && updates.ci_fix_count.is_none()
        && updates.close_finished.is_none()
        && updates.feature_name.is_none()
        && updates.consecutive_failures_epoch.is_none()
        && updates.worktree_path.is_none()
    {
        return Ok(());
    }

    // Build the full updates (including state)
    let mut full_updates = updates.clone();
    if state_changed {
        full_updates.state = Some(decision.next_state);
    }

    issue_repo
        .update_state_and_metadata(issue.id, &full_updates)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "persist failed for issue #{}: {e}",
                issue.github_issue_number
            )
        })?;

    tracing::info!(
        issue_number = issue.github_issue_number,
        from = ?issue.state,
        to = ?decision.next_state,
        "persisted decision"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::decision::Decision;
    use crate::domain::issue::Issue;
    use crate::domain::metadata_update::MetadataUpdates;
    use crate::domain::state::State;
    use std::sync::{Arc, Mutex};

    struct MockIssueRepo {
        calls: Arc<Mutex<Vec<(i64, MetadataUpdates)>>>,
    }

    type MockCalls = Arc<Mutex<Vec<(i64, MetadataUpdates)>>>;

    impl MockIssueRepo {
        fn new() -> (Self, MockCalls) {
            let calls = Arc::new(Mutex::new(vec![]));
            (
                Self {
                    calls: calls.clone(),
                },
                calls,
            )
        }
    }

    impl crate::application::port::issue_repository::IssueRepository for MockIssueRepo {
        async fn find_by_id(&self, _: i64) -> anyhow::Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_by_issue_number(&self, _: u64) -> anyhow::Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_active(&self) -> anyhow::Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn find_all(&self) -> anyhow::Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn save(&self, _: &Issue) -> anyhow::Result<i64> {
            unimplemented!()
        }
        async fn update_state(&self, _: i64, _: State) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn update(&self, _: &Issue) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn update_state_and_metadata(
            &self,
            issue_id: i64,
            updates: &MetadataUpdates,
        ) -> anyhow::Result<()> {
            self.calls.lock().unwrap().push((issue_id, updates.clone()));
            Ok(())
        }
        async fn find_by_state(&self, _: State) -> anyhow::Result<Vec<Issue>> {
            unimplemented!()
        }
    }

    fn make_issue(state: State) -> Issue {
        Issue {
            id: 1,
            github_issue_number: 42,
            state,
            worktree_path: None,
            ci_fix_count: 0,
            ci_fix_limit_notified: false,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
            feature_name: "issue-42".to_string(),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// T-3.PE.1: persist writes when state changes
    #[tokio::test]
    async fn persist_writes_on_state_change() {
        let (repo, calls) = MockIssueRepo::new();
        let issue = make_issue(State::Idle);
        let decision = Decision::new(State::InitializeRunning, MetadataUpdates::default(), vec![]);

        persist_decision(&repo, &issue, &decision)
            .await
            .expect("persist");

        let c = calls.lock().unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, 1);
        assert_eq!(c[0].1.state, Some(State::InitializeRunning));
    }

    /// T-3.PE.2: persist skips when nothing changed
    #[tokio::test]
    async fn persist_skips_when_no_change() {
        let (repo, calls) = MockIssueRepo::new();
        let issue = make_issue(State::Idle);
        let decision = Decision::stay(State::Idle);

        persist_decision(&repo, &issue, &decision)
            .await
            .expect("persist");

        let c = calls.lock().unwrap();
        assert_eq!(c.len(), 0, "should skip DB write when nothing changed");
    }

    /// T-3.PE.3: persist writes metadata even when state unchanged
    #[tokio::test]
    async fn persist_writes_metadata_without_state_change() {
        let (repo, calls) = MockIssueRepo::new();
        let issue = make_issue(State::DesignRunning);
        let updates = MetadataUpdates {
            ci_fix_count: Some(1),
            ..Default::default()
        };
        let decision = Decision::new(State::DesignRunning, updates, vec![]);

        persist_decision(&repo, &issue, &decision)
            .await
            .expect("persist");

        let c = calls.lock().unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].1.ci_fix_count, Some(1));
        // state should not be written since it didn't change
        assert!(c[0].1.state.is_none());
    }
}
