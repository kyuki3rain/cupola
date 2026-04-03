use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::application::port::issue_repository::IssueRepository;
use crate::domain::fixing_problem_kind::FixingProblemKind;
use crate::domain::issue::Issue;
use crate::domain::state::State;
use crate::domain::task_weight::TaskWeight;

use super::sqlite_connection::SqliteConnection;

pub struct SqliteIssueRepository {
    db: SqliteConnection,
}

impl SqliteIssueRepository {
    pub fn new(db: SqliteConnection) -> Self {
        Self { db }
    }
}

impl IssueRepository for SqliteIssueRepository {
    async fn find_by_id(&self, id: i64) -> Result<Option<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || -> Result<Option<Issue>> {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, design_pr_number, impl_pr_number,
                        worktree_path, retry_count, current_pid, error_message, feature_name, weight,
                        fixing_causes, created_at, updated_at
                 FROM issues WHERE id = ?1",
            )?;
            let issue = stmt
                .query_row([id], row_to_issue)
                .optional()
                .context("find_by_id query failed")?;
            Ok(issue)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn find_by_issue_number(&self, issue_number: u64) -> Result<Option<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || -> Result<Option<Issue>> {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, design_pr_number, impl_pr_number,
                        worktree_path, retry_count, current_pid, error_message, feature_name, weight,
                        fixing_causes, created_at, updated_at
                 FROM issues WHERE github_issue_number = ?1",
            )?;
            let issue = stmt
                .query_row([issue_number], row_to_issue)
                .optional()
                .context("find_by_issue_number query failed")?;
            Ok(issue)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn find_active(&self) -> Result<Vec<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, design_pr_number, impl_pr_number,
                        worktree_path, retry_count, current_pid, error_message, feature_name, weight,
                        fixing_causes, created_at, updated_at
                 FROM issues WHERE state NOT IN ('completed', 'cancelled')",
            )?;
            let issues = stmt
                .query_map([], row_to_issue)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("find_active query failed")?;
            Ok(issues)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn find_needing_process(&self) -> Result<Vec<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, design_pr_number, impl_pr_number,
                        worktree_path, retry_count, current_pid, error_message, feature_name, weight,
                        fixing_causes, created_at, updated_at
                 FROM issues WHERE state IN ('design_running', 'design_fixing', 'implementation_running', 'implementation_fixing')",
            )?;
            let issues = stmt
                .query_map([], row_to_issue)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("find_needing_process query failed")?;
            Ok(issues)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn save(&self, issue: &Issue) -> Result<i64> {
        let issue = issue.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let fixing_causes_json = serde_json::to_string(&issue.fixing_causes)
                .context("failed to serialize fixing_causes")?;
            conn.execute(
                "INSERT INTO issues (github_issue_number, state, design_pr_number, impl_pr_number,
                                     worktree_path, retry_count, current_pid, error_message, feature_name, weight,
                                     fixing_causes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    issue.github_issue_number,
                    state_to_str(&issue.state),
                    issue.design_pr_number,
                    issue.impl_pr_number,
                    issue.worktree_path,
                    issue.retry_count,
                    issue.current_pid,
                    issue.error_message,
                    issue.feature_name,
                    task_weight_to_str(issue.weight),
                    fixing_causes_json,
                ],
            )
            .context("save issue failed")?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn update_state(&self, id: i64, state: State) -> Result<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            conn.execute(
                "UPDATE issues SET state = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![state_to_str(&state), id],
            )
            .context("update_state failed")?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn update(&self, issue: &Issue) -> Result<()> {
        let issue = issue.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let fixing_causes_json = serde_json::to_string(&issue.fixing_causes)
                .context("failed to serialize fixing_causes")?;
            conn.execute(
                "UPDATE issues SET state = ?1, design_pr_number = ?2, impl_pr_number = ?3,
                                   worktree_path = ?4, retry_count = ?5, current_pid = ?6,
                                   error_message = ?7, feature_name = ?8, weight = ?9,
                                   fixing_causes = ?10, updated_at = datetime('now')
                 WHERE id = ?11",
                rusqlite::params![
                    state_to_str(&issue.state),
                    issue.design_pr_number,
                    issue.impl_pr_number,
                    issue.worktree_path,
                    issue.retry_count,
                    issue.current_pid,
                    issue.error_message,
                    issue.feature_name,
                    task_weight_to_str(issue.weight),
                    fixing_causes_json,
                    issue.id,
                ],
            )
            .context("update issue failed")?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn reset_for_restart(&self, id: i64) -> Result<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            conn.execute(
                "UPDATE issues
                 SET state = 'idle',
                     retry_count = 0,
                     current_pid = NULL,
                     error_message = NULL,
                     fixing_causes = '[]',
                     updated_at = datetime('now')
                 WHERE id = ?1",
                [id],
            )
            .context("reset_for_restart failed")?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn find_by_state(&self, state: State) -> Result<Vec<Issue>> {
        let db = self.db.clone();
        let state_str = state_to_str(&state).to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, design_pr_number, impl_pr_number,
                        worktree_path, retry_count, current_pid, error_message, feature_name, weight,
                        fixing_causes, created_at, updated_at
                 FROM issues WHERE state = ?1",
            )?;
            let issues = stmt
                .query_map([state_str], row_to_issue)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("find_by_state query failed")?;
            Ok(issues)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }
}

fn state_to_str(state: &State) -> &'static str {
    match state {
        State::Idle => "idle",
        State::Initialized => "initialized",
        State::DesignRunning => "design_running",
        State::DesignReviewWaiting => "design_review_waiting",
        State::DesignFixing => "design_fixing",
        State::ImplementationRunning => "implementation_running",
        State::ImplementationReviewWaiting => "implementation_review_waiting",
        State::ImplementationFixing => "implementation_fixing",
        State::Completed => "completed",
        State::Cancelled => "cancelled",
    }
}

pub fn str_to_state(col_idx: usize, s: &str) -> rusqlite::Result<State> {
    match s {
        "idle" => Ok(State::Idle),
        "initialized" => Ok(State::Initialized),
        "design_running" => Ok(State::DesignRunning),
        "design_review_waiting" => Ok(State::DesignReviewWaiting),
        "design_fixing" => Ok(State::DesignFixing),
        "implementation_running" => Ok(State::ImplementationRunning),
        "implementation_review_waiting" => Ok(State::ImplementationReviewWaiting),
        "implementation_fixing" => Ok(State::ImplementationFixing),
        "completed" => Ok(State::Completed),
        "cancelled" => Ok(State::Cancelled),
        _ => Err(rusqlite::Error::InvalidColumnType(
            col_idx,
            s.to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

fn task_weight_to_str(weight: TaskWeight) -> &'static str {
    match weight {
        TaskWeight::Light => "light",
        TaskWeight::Medium => "medium",
        TaskWeight::Heavy => "heavy",
    }
}

fn str_to_task_weight(col_idx: usize, s: &str) -> rusqlite::Result<TaskWeight> {
    match s {
        "light" => Ok(TaskWeight::Light),
        "medium" => Ok(TaskWeight::Medium),
        "heavy" => Ok(TaskWeight::Heavy),
        _ => Err(rusqlite::Error::InvalidColumnType(
            col_idx,
            s.to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

fn row_to_issue(row: &rusqlite::Row) -> rusqlite::Result<Issue> {
    let state_str: String = row.get(2)?;
    let weight_str: String = row.get(10)?;
    let fixing_causes_json: String = row.get(11)?;
    let created_str: String = row.get(12)?;
    let updated_str: String = row.get(13)?;

    let fixing_causes: Vec<FixingProblemKind> =
        serde_json::from_str::<Vec<FixingProblemKind>>(&fixing_causes_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(e))
        })?;

    Ok(Issue {
        id: row.get(0)?,
        github_issue_number: row.get(1)?,
        state: str_to_state(2, &state_str)?,
        design_pr_number: row.get(3)?,
        impl_pr_number: row.get(4)?,
        worktree_path: row.get(5)?,
        retry_count: row.get(6)?,
        current_pid: row.get(7)?,
        error_message: row.get(8)?,
        feature_name: row.get(9)?,
        weight: str_to_task_weight(10, &weight_str)?,
        fixing_causes,
        created_at: parse_sqlite_datetime(12, &created_str)?,
        updated_at: parse_sqlite_datetime(13, &updated_str)?,
    })
}

fn parse_sqlite_datetime(col_idx: usize, s: &str) -> rusqlite::Result<DateTime<Utc>> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|naive| naive.and_utc())
        .map_err(|_| {
            rusqlite::Error::InvalidColumnType(col_idx, s.to_owned(), rusqlite::types::Type::Text)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::outbound::sqlite_connection::SqliteConnection;

    fn setup() -> (SqliteConnection, SqliteIssueRepository) {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init schema");
        let repo = SqliteIssueRepository::new(db.clone());
        (db, repo)
    }

    fn new_issue(issue_number: u64) -> Issue {
        Issue {
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
            fixing_causes: vec![],
            weight: TaskWeight::Medium,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn save_and_find_by_id() {
        let (_db, repo) = setup();
        let issue = new_issue(42);

        let id = repo.save(&issue).await.expect("save");
        let found = repo.find_by_id(id).await.expect("find");
        let found = found.expect("should exist");

        assert_eq!(found.id, id);
        assert_eq!(found.github_issue_number, 42);
        assert_eq!(found.state, State::Idle);
        assert_eq!(found.weight, TaskWeight::Medium);
    }

    #[tokio::test]
    async fn find_by_issue_number() {
        let (_db, repo) = setup();
        repo.save(&new_issue(99)).await.expect("save");

        let found = repo.find_by_issue_number(99).await.expect("find");
        assert!(found.is_some());

        let not_found = repo.find_by_issue_number(100).await.expect("find");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn find_active_excludes_terminal() {
        let (_db, repo) = setup();
        let id1 = repo.save(&new_issue(1)).await.expect("save");
        let id2 = repo.save(&new_issue(2)).await.expect("save");
        repo.save(&new_issue(3)).await.expect("save");

        repo.update_state(id1, State::DesignRunning)
            .await
            .expect("update");
        repo.update_state(id2, State::Completed)
            .await
            .expect("update");

        let active = repo.find_active().await.expect("find_active");
        let numbers: Vec<u64> = active.iter().map(|i| i.github_issue_number).collect();
        assert!(numbers.contains(&1));
        assert!(!numbers.contains(&2));
        assert!(numbers.contains(&3));
    }

    #[tokio::test]
    async fn find_needing_process() {
        let (_db, repo) = setup();
        let id1 = repo.save(&new_issue(10)).await.expect("save");
        let id2 = repo.save(&new_issue(20)).await.expect("save");
        repo.save(&new_issue(30)).await.expect("save");

        repo.update_state(id1, State::DesignRunning)
            .await
            .expect("update");
        repo.update_state(id2, State::DesignReviewWaiting)
            .await
            .expect("update");

        let needing = repo.find_needing_process().await.expect("find");
        assert_eq!(needing.len(), 1);
        assert_eq!(needing[0].github_issue_number, 10);
    }

    #[tokio::test]
    async fn update_state() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(50)).await.expect("save");

        repo.update_state(id, State::Initialized)
            .await
            .expect("update_state");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.state, State::Initialized);
    }

    #[tokio::test]
    async fn update_full_issue() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(60)).await.expect("save");

        let mut issue = repo.find_by_id(id).await.expect("find").expect("exists");
        issue.state = State::DesignRunning;
        issue.worktree_path = Some("/tmp/wt".to_string());
        issue.retry_count = 2;

        repo.update(&issue).await.expect("update");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.state, State::DesignRunning);
        assert_eq!(found.worktree_path.as_deref(), Some("/tmp/wt"));
        assert_eq!(found.retry_count, 2);
    }

    #[tokio::test]
    async fn reset_for_restart_clears_transient_fields() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(70)).await.expect("save");

        repo.update_state(id, State::Cancelled)
            .await
            .expect("update");
        repo.reset_for_restart(id).await.expect("reset");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.state, State::Idle);
        assert_eq!(found.retry_count, 0);
        assert!(found.current_pid.is_none());
        assert!(found.error_message.is_none());
    }

    #[tokio::test]
    async fn reset_for_restart_preserves_resource_fields() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(71)).await.expect("save");

        // Set resource fields before cancelling
        let mut issue = repo.find_by_id(id).await.expect("find").expect("exists");
        issue.state = State::Cancelled;
        issue.design_pr_number = Some(42);
        issue.impl_pr_number = Some(99);
        issue.worktree_path = Some("/tmp/worktrees/71".to_string());
        issue.feature_name = Some("my-feature".to_string());
        issue.retry_count = 3;
        issue.error_message = Some("some error".to_string());
        repo.update(&issue).await.expect("update");

        repo.reset_for_restart(id).await.expect("reset");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        // Transient fields reset
        assert_eq!(found.state, State::Idle);
        assert_eq!(found.retry_count, 0);
        assert!(found.error_message.is_none());
        // Resource fields preserved
        assert_eq!(found.design_pr_number, Some(42));
        assert_eq!(found.impl_pr_number, Some(99));
        assert_eq!(found.worktree_path.as_deref(), Some("/tmp/worktrees/71"));
        assert_eq!(found.feature_name.as_deref(), Some("my-feature"));
    }

    #[tokio::test]
    async fn find_by_state_returns_only_matching() {
        let (_db, repo) = setup();
        let id1 = repo.save(&new_issue(100)).await.expect("save");
        let id2 = repo.save(&new_issue(101)).await.expect("save");
        let id3 = repo.save(&new_issue(102)).await.expect("save");

        repo.update_state(id1, State::Cancelled)
            .await
            .expect("update");
        repo.update_state(id2, State::Cancelled)
            .await
            .expect("update");
        repo.update_state(id3, State::Completed)
            .await
            .expect("update");

        let cancelled = repo
            .find_by_state(State::Cancelled)
            .await
            .expect("find_by_state");
        assert_eq!(cancelled.len(), 2);
        let numbers: Vec<u64> = cancelled.iter().map(|i| i.github_issue_number).collect();
        assert!(numbers.contains(&100));
        assert!(numbers.contains(&101));
        assert!(!numbers.contains(&102));
    }

    #[tokio::test]
    async fn find_by_state_returns_empty_when_none_match() {
        let (_db, repo) = setup();
        repo.save(&new_issue(200)).await.expect("save");

        let cancelled = repo
            .find_by_state(State::Cancelled)
            .await
            .expect("find_by_state");
        assert!(cancelled.is_empty());
    }

    #[tokio::test]
    async fn fixing_causes_persist_and_load() {
        use crate::domain::fixing_problem_kind::FixingProblemKind;

        let (_db, repo) = setup();
        let id = repo.save(&new_issue(80)).await.expect("save");

        let mut issue = repo.find_by_id(id).await.expect("find").expect("exists");
        assert!(issue.fixing_causes.is_empty());

        issue.fixing_causes = vec![FixingProblemKind::CiFailure, FixingProblemKind::Conflict];
        repo.update(&issue).await.expect("update");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.fixing_causes.len(), 2);
        assert_eq!(found.fixing_causes[0], FixingProblemKind::CiFailure);
        assert_eq!(found.fixing_causes[1], FixingProblemKind::Conflict);
    }

    #[test]
    fn state_roundtrip() {
        let states = [
            State::Idle,
            State::Initialized,
            State::DesignRunning,
            State::DesignReviewWaiting,
            State::DesignFixing,
            State::ImplementationRunning,
            State::ImplementationReviewWaiting,
            State::ImplementationFixing,
            State::Completed,
            State::Cancelled,
        ];
        for state in states {
            assert_eq!(str_to_state(0, state_to_str(&state)).unwrap(), state);
        }
    }

    #[test]
    fn str_to_state_known_states() {
        let cases = [
            ("idle", State::Idle),
            ("initialized", State::Initialized),
            ("design_running", State::DesignRunning),
            ("design_review_waiting", State::DesignReviewWaiting),
            ("design_fixing", State::DesignFixing),
            ("implementation_running", State::ImplementationRunning),
            (
                "implementation_review_waiting",
                State::ImplementationReviewWaiting,
            ),
            ("implementation_fixing", State::ImplementationFixing),
            ("completed", State::Completed),
            ("cancelled", State::Cancelled),
        ];
        for (s, expected) in cases {
            assert_eq!(str_to_state(2, s).expect(s), expected);
        }
    }

    #[test]
    fn str_to_state_unknown_returns_err() {
        assert!(str_to_state(2, "unknown_state").is_err());
        assert!(str_to_state(2, "").is_err());
        assert!(str_to_state(2, "IDLE").is_err());
    }

    #[test]
    fn task_weight_roundtrip() {
        let weights = [TaskWeight::Light, TaskWeight::Medium, TaskWeight::Heavy];
        for weight in weights {
            assert_eq!(
                str_to_task_weight(0, task_weight_to_str(weight)).unwrap(),
                weight
            );
        }
    }

    #[test]
    fn str_to_task_weight_unknown_returns_err() {
        assert!(str_to_task_weight(10, "unknown").is_err());
        assert!(str_to_task_weight(10, "").is_err());
        assert!(str_to_task_weight(10, "MEDIUM").is_err());
    }

    #[tokio::test]
    async fn weight_persists_and_loads() {
        let (_db, repo) = setup();

        let mut issue = new_issue(500);
        issue.weight = TaskWeight::Heavy;
        let id = repo.save(&issue).await.expect("save");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.weight, TaskWeight::Heavy);

        // Update to Light
        let mut loaded = found;
        loaded.weight = TaskWeight::Light;
        repo.update(&loaded).await.expect("update");

        let found2 = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found2.weight, TaskWeight::Light);
    }

    #[tokio::test]
    async fn unknown_weight_in_db_returns_err() {
        let (db, repo) = setup();
        {
            let conn = db.conn().lock().expect("lock");
            conn.execute(
                "INSERT INTO issues (github_issue_number, state, weight, fixing_causes, created_at, updated_at)
                 VALUES (999, 'idle', 'unknown_weight', '[]', datetime('now'), datetime('now'))",
                [],
            )
            .expect("insert corrupt row");
        }

        let result = repo.find_by_issue_number(999).await;
        assert!(result.is_err(), "unknown weight should return Err");
    }

    #[test]
    fn parse_sqlite_datetime_valid() {
        let result = parse_sqlite_datetime(12, "2024-01-15 10:30:00");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2024-01-15 10:30:00"
        );
    }

    #[test]
    fn parse_sqlite_datetime_invalid() {
        assert!(parse_sqlite_datetime(12, "not-a-date").is_err());
        assert!(parse_sqlite_datetime(12, "").is_err());
        assert!(parse_sqlite_datetime(12, "2024/01/15 10:30:00").is_err());
    }

    #[tokio::test]
    async fn find_active_with_corrupt_state_returns_err() {
        let (db, repo) = setup();
        {
            let conn = db.conn().lock().expect("lock");
            conn.execute(
                "INSERT INTO issues (github_issue_number, state, weight, fixing_causes, created_at, updated_at)
                 VALUES (998, 'unknown_corrupt_state', 'medium', '[]', datetime('now'), datetime('now'))",
                [],
            )
            .expect("insert corrupt row");
        }

        let result = repo.find_active().await;
        assert!(
            result.is_err(),
            "find_active should return Err for corrupt state"
        );
    }

    #[tokio::test]
    async fn find_active_with_corrupt_fixing_causes_returns_err() {
        let (db, repo) = setup();
        {
            let conn = db.conn().lock().expect("lock");
            conn.execute(
                "INSERT INTO issues (github_issue_number, state, weight, fixing_causes, created_at, updated_at)
                 VALUES (997, 'idle', 'medium', 'not_json', datetime('now'), datetime('now'))",
                [],
            )
            .expect("insert corrupt row");
        }

        let result = repo.find_active().await;
        assert!(
            result.is_err(),
            "find_active should return Err for corrupt fixing_causes"
        );
    }

    #[tokio::test]
    async fn reset_for_restart_clears_fixing_causes() {
        use crate::domain::fixing_problem_kind::FixingProblemKind;

        let (_db, repo) = setup();
        let id = repo.save(&new_issue(300)).await.expect("save");

        let mut issue = repo.find_by_id(id).await.expect("find").expect("exists");
        issue.fixing_causes = vec![FixingProblemKind::CiFailure];
        repo.update(&issue).await.expect("update");

        repo.reset_for_restart(id).await.expect("reset");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert!(
            found.fixing_causes.is_empty(),
            "fixing_causes should be empty after reset"
        );
    }
}
