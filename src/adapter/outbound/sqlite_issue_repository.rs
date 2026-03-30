use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::application::port::issue_repository::IssueRepository;
use crate::domain::fixing_problem_kind::FixingProblemKind;
use crate::domain::issue::Issue;
use crate::domain::state::State;

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
            let conn = db.conn().lock().expect("mutex poisoned");
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, design_pr_number, impl_pr_number,
                        worktree_path, retry_count, current_pid, error_message, feature_name,
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
        .expect("spawn_blocking panicked")
    }

    async fn find_by_issue_number(&self, issue_number: u64) -> Result<Option<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || -> Result<Option<Issue>> {
            let conn = db.conn().lock().expect("mutex poisoned");
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, design_pr_number, impl_pr_number,
                        worktree_path, retry_count, current_pid, error_message, feature_name,
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
        .expect("spawn_blocking panicked")
    }

    async fn find_active(&self) -> Result<Vec<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn().lock().expect("mutex poisoned");
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, design_pr_number, impl_pr_number,
                        worktree_path, retry_count, current_pid, error_message, feature_name,
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
        .expect("spawn_blocking panicked")
    }

    async fn find_needing_process(&self) -> Result<Vec<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn().lock().expect("mutex poisoned");
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, design_pr_number, impl_pr_number,
                        worktree_path, retry_count, current_pid, error_message, feature_name,
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
        .expect("spawn_blocking panicked")
    }

    async fn save(&self, issue: &Issue) -> Result<i64> {
        let issue = issue.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn().lock().expect("mutex poisoned");
            let fixing_causes_json =
                serde_json::to_string(&issue.fixing_causes).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "INSERT INTO issues (github_issue_number, state, design_pr_number, impl_pr_number,
                                     worktree_path, retry_count, current_pid, error_message, feature_name,
                                     fixing_causes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                    fixing_causes_json,
                ],
            )
            .context("save issue failed")?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .expect("spawn_blocking panicked")
    }

    async fn update_state(&self, id: i64, state: State) -> Result<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn().lock().expect("mutex poisoned");
            conn.execute(
                "UPDATE issues SET state = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![state_to_str(&state), id],
            )
            .context("update_state failed")?;
            Ok(())
        })
        .await
        .expect("spawn_blocking panicked")
    }

    async fn update(&self, issue: &Issue) -> Result<()> {
        let issue = issue.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn().lock().expect("mutex poisoned");
            let fixing_causes_json =
                serde_json::to_string(&issue.fixing_causes).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "UPDATE issues SET state = ?1, design_pr_number = ?2, impl_pr_number = ?3,
                                   worktree_path = ?4, retry_count = ?5, current_pid = ?6,
                                   error_message = ?7, feature_name = ?8, fixing_causes = ?9,
                                   updated_at = datetime('now')
                 WHERE id = ?10",
                rusqlite::params![
                    state_to_str(&issue.state),
                    issue.design_pr_number,
                    issue.impl_pr_number,
                    issue.worktree_path,
                    issue.retry_count,
                    issue.current_pid,
                    issue.error_message,
                    issue.feature_name,
                    fixing_causes_json,
                    issue.id,
                ],
            )
            .context("update issue failed")?;
            Ok(())
        })
        .await
        .expect("spawn_blocking panicked")
    }

    async fn reset_for_restart(&self, id: i64) -> Result<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn().lock().expect("mutex poisoned");
            conn.execute(
                "UPDATE issues
                 SET state = 'idle',
                     design_pr_number = NULL,
                     impl_pr_number = NULL,
                     worktree_path = NULL,
                     retry_count = 0,
                     current_pid = NULL,
                     error_message = NULL,
                     feature_name = NULL,
                     updated_at = datetime('now')
                 WHERE id = ?1",
                [id],
            )
            .context("reset_for_restart failed")?;
            Ok(())
        })
        .await
        .expect("spawn_blocking panicked")
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

pub fn str_to_state(s: &str) -> State {
    match s {
        "idle" => State::Idle,
        "initialized" => State::Initialized,
        "design_running" => State::DesignRunning,
        "design_review_waiting" => State::DesignReviewWaiting,
        "design_fixing" => State::DesignFixing,
        "implementation_running" => State::ImplementationRunning,
        "implementation_review_waiting" => State::ImplementationReviewWaiting,
        "implementation_fixing" => State::ImplementationFixing,
        "completed" => State::Completed,
        "cancelled" => State::Cancelled,
        _ => State::Idle,
    }
}

fn row_to_issue(row: &rusqlite::Row) -> rusqlite::Result<Issue> {
    let state_str: String = row.get(2)?;
    let fixing_causes_json: String = row.get(10).unwrap_or_else(|_| "[]".to_string());
    let created_str: String = row.get(11)?;
    let updated_str: String = row.get(12)?;

    let fixing_causes: Vec<FixingProblemKind> =
        serde_json::from_str(&fixing_causes_json).unwrap_or_default();

    Ok(Issue {
        id: row.get(0)?,
        github_issue_number: row.get(1)?,
        state: str_to_state(&state_str),
        design_pr_number: row.get(3)?,
        impl_pr_number: row.get(4)?,
        worktree_path: row.get(5)?,
        retry_count: row.get(6)?,
        current_pid: row.get(7)?,
        error_message: row.get(8)?,
        feature_name: row.get(9)?,
        fixing_causes,
        created_at: parse_sqlite_datetime(&created_str),
        updated_at: parse_sqlite_datetime(&updated_str),
    })
}

fn parse_sqlite_datetime(s: &str) -> DateTime<Utc> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|naive| naive.and_utc())
        .unwrap_or_default()
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
    async fn reset_for_restart() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(70)).await.expect("save");

        repo.update_state(id, State::Cancelled)
            .await
            .expect("update");
        repo.reset_for_restart(id).await.expect("reset");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.state, State::Idle);
        assert!(found.design_pr_number.is_none());
        assert!(found.impl_pr_number.is_none());
        assert!(found.worktree_path.is_none());
        assert_eq!(found.retry_count, 0);
        assert!(found.current_pid.is_none());
        assert!(found.error_message.is_none());
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
            assert_eq!(str_to_state(state_to_str(&state)), state);
        }
    }
}
