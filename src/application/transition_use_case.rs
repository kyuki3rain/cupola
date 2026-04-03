use anyhow::{Context as _, Result};
use rust_i18n::t;

use crate::application::port::git_worktree::GitWorktree;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::domain::config::Config;
use crate::domain::event::Event;
use crate::domain::issue::Issue;
use crate::domain::state::State;
use crate::domain::state_machine::StateMachine;
use crate::domain::task_weight::TaskWeight;

pub struct TransitionUseCase<'a, G: GitHubClient, I: IssueRepository, W: GitWorktree> {
    pub github: &'a G,
    pub issue_repo: &'a I,
    pub worktree: &'a W,
    pub config: &'a Config,
}

impl<G: GitHubClient, I: IssueRepository, W: GitWorktree> TransitionUseCase<'_, G, I, W> {
    /// Apply an event to an issue, performing state transition and side effects.
    /// Returns the new state.
    pub async fn apply(&self, issue: &mut Issue, event: &Event) -> Result<State> {
        let new_state = StateMachine::transition(&issue.state, event)?;
        let old_state = issue.state;

        // Update state in DB first (critical for terminal states)
        self.issue_repo.update_state(issue.id, new_state).await?;
        issue.state = new_state;

        if old_state != new_state {
            tracing::info!(
                issue_number = issue.github_issue_number,
                from = ?old_state,
                to = ?new_state,
                event = ?event,
                retry_count = issue.retry_count,
                "state transition"
            );
        }

        // Reset retry_count on normal (non-retry) transitions
        if old_state != new_state && !matches!(event, Event::ProcessFailed) {
            issue.retry_count = 0;
            self.issue_repo.update(issue).await?;
        }

        // Execute side effects
        self.execute_side_effects(issue, old_state, new_state, event)
            .await?;

        Ok(new_state)
    }

    /// Handle initial issue detection (Idle → Initialized).
    pub async fn handle_issue_detected(&self, issue_number: u64) -> Result<Issue> {
        // Check for existing record
        if let Some(existing) = self.issue_repo.find_by_issue_number(issue_number).await? {
            if existing.state.is_terminal() {
                // Reset for restart
                self.issue_repo.reset_for_restart(existing.id).await?;
                let mut reset = self
                    .issue_repo
                    .find_by_id(existing.id)
                    .await?
                    .context("issue not found after reset_for_restart")?;
                // Apply IssueDetected to go Idle → Initialized
                let event = Event::IssueDetected { issue_number };
                self.apply(&mut reset, &event).await?;
                return Ok(reset);
            }
            // Non-terminal existing record — skip
            return Ok(existing);
        }

        // New issue
        let mut issue = Issue {
            id: 0,
            github_issue_number: issue_number,
            state: State::Idle,
            design_pr_number: None,
            impl_pr_number: None,
            worktree_path: None,
            retry_count: 0,
            current_pid: None,
            error_message: None,
            feature_name: None,
            weight: TaskWeight::Medium,
            fixing_causes: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let id = self.issue_repo.save(&issue).await?;
        issue.id = id;

        let event = Event::IssueDetected { issue_number };
        self.apply(&mut issue, &event).await?;

        Ok(issue)
    }

    async fn execute_side_effects(
        &self,
        issue: &mut Issue,
        _old_state: State,
        new_state: State,
        event: &Event,
    ) -> Result<()> {
        match (new_state, event) {
            // Initialized → DesignRunning side effects are handled by polling step 2
            // (initialization flow), not here.

            // DesignReviewWaiting → ImplementationRunning
            (State::ImplementationRunning, Event::DesignPrMerged) => {
                if let Some(ref wt) = issue.worktree_path {
                    let wt_path = std::path::Path::new(wt);
                    let branch = format!("cupola/{}/main", issue.github_issue_number);
                    self.worktree.checkout(wt_path, &branch)?;
                    self.worktree.pull(wt_path)?;
                }
                self.github
                    .comment_on_issue(
                        issue.github_issue_number,
                        &t!(
                            "issue_comment.implementation_starting",
                            locale = &self.config.language
                        ),
                    )
                    .await?;
            }

            // Completed
            (State::Completed, _) => {
                self.github
                    .comment_on_issue(
                        issue.github_issue_number,
                        &t!(
                            "issue_comment.all_completed",
                            locale = &self.config.language
                        ),
                    )
                    .await?;
                let _ = self.github.close_issue(issue.github_issue_number).await;
                self.cleanup(issue).await;
            }

            // Cancelled
            (State::Cancelled, Event::IssueClosed) => {
                self.cleanup(issue).await;
                let _ = self
                    .github
                    .comment_on_issue(
                        issue.github_issue_number,
                        &t!("issue_comment.cleanup_done", locale = &self.config.language),
                    )
                    .await;
            }

            (State::Cancelled, Event::RetryExhausted) => {
                let unknown = t!(
                    "issue_comment.unknown_error",
                    locale = &self.config.language
                );
                let msg = t!(
                    "issue_comment.retry_exhausted",
                    locale = &self.config.language,
                    count = issue.retry_count,
                    error = issue.error_message.as_deref().unwrap_or(&unknown)
                );
                // worktree・ブランチは保持する（reopen 時に再利用可能）
                let _ = self.github.close_issue(issue.github_issue_number).await;
                let _ = self
                    .github
                    .comment_on_issue(issue.github_issue_number, &msg)
                    .await;
            }

            // ProcessFailed (retry)
            (_, Event::ProcessFailed) => {
                issue.retry_count += 1;
                self.issue_repo.update(issue).await?;
            }

            _ => {}
        }

        Ok(())
    }

    async fn cleanup(&self, issue: &mut Issue) {
        if let Some(ref wt) = issue.worktree_path {
            let path = std::path::Path::new(wt);
            match self.worktree.remove(path) {
                Ok(()) => {
                    issue.worktree_path = None;
                    let _ = self.issue_repo.update(issue).await;
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "failed to remove worktree, keeping path in DB");
                }
            }
        }
        let n = issue.github_issue_number;
        let _ = self.worktree.delete_branch(&format!("cupola/{n}/main"));
        let _ = self.worktree.delete_branch(&format!("cupola/{n}/design"));
    }
}

/// Sort events so IssueClosed is processed first for a given issue.
pub fn prioritize_events(events: &mut [(i64, Event)]) {
    events.sort_by_key(|(_, event)| match event {
        Event::IssueClosed => 0,
        _ => 1,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prioritize_events_puts_issue_closed_first() {
        let mut events = vec![
            (1, Event::ProcessFailed),
            (1, Event::IssueClosed),
            (2, Event::ProcessCompletedWithPr),
        ];
        prioritize_events(&mut events);
        assert_eq!(events[0].1, Event::IssueClosed);
    }

    #[test]
    fn prioritize_events_stable_when_no_close() {
        let mut events = vec![
            (1, Event::ProcessFailed),
            (2, Event::ProcessCompletedWithPr),
        ];
        prioritize_events(&mut events);
        // Both have same priority (1), order preserved
        assert_eq!(events[0].0, 1);
        assert_eq!(events[1].0, 2);
    }
}
