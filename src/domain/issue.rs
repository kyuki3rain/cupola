use chrono::{DateTime, Utc};

use crate::domain::state::State;
use crate::domain::task_weight::TaskWeight;

#[derive(Debug, Clone)]
pub struct Issue {
    pub id: i64,
    pub github_issue_number: u64,
    pub state: State,
    pub feature_name: String,
    pub weight: TaskWeight,
    pub worktree_path: Option<String>,
    pub ci_fix_count: u32,
    pub ci_fix_limit_notified: bool,
    pub close_finished: bool,
    pub consecutive_failures_epoch: Option<DateTime<Utc>>,
    /// 最後に fixing トリガーとして採用した PR レベルレビューの submittedAt タイムスタンプ。
    /// NULL の場合は未処理（すべての PR レベルレビューを新規として扱う）。
    pub last_pr_review_submitted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Issue {
    pub fn new(github_issue_number: u64, feature_name: String) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            github_issue_number,
            state: State::Idle,
            feature_name,
            weight: TaskWeight::Medium,
            worktree_path: None,
            ci_fix_count: 0,
            ci_fix_limit_notified: false,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-1.I.1: verify field set (compile-time check via struct literal)
    #[test]
    fn issue_has_correct_fields() {
        let now = Utc::now();
        let issue = Issue {
            id: 1,
            github_issue_number: 42,
            state: State::Idle,
            feature_name: "test".to_string(),
            weight: TaskWeight::Medium,
            worktree_path: None,
            ci_fix_count: 0,
            ci_fix_limit_notified: false,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
            created_at: now,
            updated_at: now,
        };
        assert_eq!(issue.id, 1);
        assert_eq!(issue.github_issue_number, 42);
        assert_eq!(issue.state, State::Idle);
        assert_eq!(issue.ci_fix_count, 0);
        assert!(!issue.close_finished);
        assert!(issue.consecutive_failures_epoch.is_none());
    }

    /// T-1.I.2: default-constructed issue has expected field values
    #[test]
    fn default_constructed_issue_has_expected_values() {
        let issue = Issue::new(1, "issue-1".to_string());
        assert_eq!(issue.state, State::Idle);
        assert_eq!(issue.ci_fix_count, 0);
        assert!(!issue.close_finished);
        assert!(issue.consecutive_failures_epoch.is_none());
    }
}
