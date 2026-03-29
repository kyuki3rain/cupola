use chrono::{DateTime, Utc};

use crate::domain::state::State;

#[derive(Debug, Clone)]
pub struct ExecutionLog {
    pub id: i64,
    pub issue_id: i64,
    pub state: State,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub structured_output: Option<String>,
    pub error_message: Option<String>,
}
