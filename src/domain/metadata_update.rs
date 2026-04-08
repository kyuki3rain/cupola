use chrono::{DateTime, Utc};

use crate::domain::issue::Issue;
use crate::domain::state::State;
use crate::domain::task_weight::TaskWeight;

/// Sparse update set produced by `decide`. `None` fields are left unchanged.
#[derive(Debug, Clone, Default)]
pub struct MetadataUpdates {
    pub state: Option<State>,
    pub weight: Option<TaskWeight>,
    pub ci_fix_count: Option<u32>,
    pub close_finished: Option<bool>,
    pub feature_name: Option<String>,
    pub consecutive_failures_epoch: Option<Option<DateTime<Utc>>>,
    pub worktree_path: Option<Option<String>>,
}

impl MetadataUpdates {
    /// Apply non-None fields to the issue in place
    pub fn apply_to(&self, issue: &mut Issue) {
        if let Some(state) = self.state {
            issue.state = state;
        }
        if let Some(weight) = self.weight {
            issue.weight = weight;
        }
        if let Some(ci_fix_count) = self.ci_fix_count {
            issue.ci_fix_count = ci_fix_count;
        }
        if let Some(close_finished) = self.close_finished {
            issue.close_finished = close_finished;
        }
        if let Some(ref feature_name) = self.feature_name {
            issue.feature_name = feature_name.clone();
        }
        if let Some(epoch) = self.consecutive_failures_epoch {
            issue.consecutive_failures_epoch = epoch;
        }
        if let Some(ref worktree_path) = self.worktree_path {
            issue.worktree_path = worktree_path.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_issue() -> Issue {
        Issue::new(1, "test".to_string())
    }

    /// T-1.M.1: struct has all Option<T> fields
    #[test]
    fn metadata_updates_has_correct_fields() {
        let m = MetadataUpdates {
            state: Some(State::DesignRunning),
            weight: Some(TaskWeight::Heavy),
            ci_fix_count: Some(2),
            close_finished: Some(true),
            feature_name: Some("feat".to_string()),
            consecutive_failures_epoch: Some(Some(Utc::now())),
            worktree_path: Some(Some("/tmp/wt".to_string())),
        };
        assert!(m.state.is_some());
        assert!(m.weight.is_some());
        assert!(m.ci_fix_count.is_some());
        assert!(m.close_finished.is_some());
        assert!(m.feature_name.is_some());
        assert!(m.consecutive_failures_epoch.is_some());
        assert!(m.worktree_path.is_some());
    }

    /// T-1.M.2: default is all-None
    #[test]
    fn metadata_updates_default_is_all_none() {
        let m = MetadataUpdates::default();
        assert!(m.state.is_none());
        assert!(m.weight.is_none());
        assert!(m.ci_fix_count.is_none());
        assert!(m.close_finished.is_none());
        assert!(m.feature_name.is_none());
        assert!(m.consecutive_failures_epoch.is_none());
        assert!(m.worktree_path.is_none());
    }

    /// T-1.M.3: apply_to only mutates fields that are Some
    #[test]
    fn apply_to_only_mutates_some_fields() {
        let mut issue = make_issue();
        issue.ci_fix_count = 5;
        issue.weight = TaskWeight::Heavy;

        let updates = MetadataUpdates {
            state: Some(State::DesignRunning),
            ci_fix_count: Some(0),
            ..Default::default()
        };
        updates.apply_to(&mut issue);

        assert_eq!(issue.state, State::DesignRunning);
        assert_eq!(issue.ci_fix_count, 0);
        // weight was not in updates, should be unchanged
        assert_eq!(issue.weight, TaskWeight::Heavy);
        // close_finished was not in updates, should be unchanged
        assert!(!issue.close_finished);
    }

    #[test]
    fn apply_to_can_set_worktree_path_to_none() {
        let mut issue = make_issue();
        issue.worktree_path = Some("/tmp/wt".to_string());

        let updates = MetadataUpdates {
            worktree_path: Some(None),
            ..Default::default()
        };
        updates.apply_to(&mut issue);
        assert!(issue.worktree_path.is_none());
    }

    #[test]
    fn apply_to_can_set_consecutive_failures_epoch() {
        let mut issue = make_issue();
        let epoch = Utc::now();

        let updates = MetadataUpdates {
            consecutive_failures_epoch: Some(Some(epoch)),
            ..Default::default()
        };
        updates.apply_to(&mut issue);
        assert!(issue.consecutive_failures_epoch.is_some());
    }
}
