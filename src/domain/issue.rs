use chrono::{DateTime, Utc};

use crate::domain::fixing_problem_kind::FixingProblemKind;
use crate::domain::state::State;
use crate::domain::task_weight::TaskWeight;

#[derive(Debug, Clone)]
pub struct Issue {
    pub id: i64,
    pub github_issue_number: u64,
    pub state: State,
    pub design_pr_number: Option<u64>,
    pub impl_pr_number: Option<u64>,
    pub worktree_path: Option<String>,
    pub retry_count: u32,
    pub ci_fix_count: u32,
    pub current_pid: Option<u32>,
    pub error_message: Option<String>,
    pub feature_name: Option<String>,
    pub weight: TaskWeight,
    pub fixing_causes: Vec<FixingProblemKind>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
