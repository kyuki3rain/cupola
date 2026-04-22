use anyhow::{Context, Result};

use crate::application::port::execution_log_repository::ExecutionLogRepository;
use crate::domain::execution_log::ExecutionLog;
use crate::domain::state::State;

use super::sqlite_connection::SqliteConnection;
use super::sqlite_helpers::{parse_sqlite_datetime, str_to_state};

pub struct SqliteExecutionLogRepository {
    db: SqliteConnection,
}

impl SqliteExecutionLogRepository {
    pub fn new(db: SqliteConnection) -> Self {
        Self { db }
    }
}

impl ExecutionLogRepository for SqliteExecutionLogRepository {
    async fn record_start(&self, issue_id: i64, state: State) -> Result<i64> {
        let db = self.db.clone();
        let state_str = state.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            conn.execute(
                "INSERT INTO execution_log (issue_id, state) VALUES (?1, ?2)",
                rusqlite::params![issue_id, state_str],
            )
            .context("record_start failed")?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .map_err(|e| {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            anyhow::anyhow!("spawn_blocking task failed: {e}")
        })?
    }

    async fn record_finish(
        &self,
        log_id: i64,
        exit_code: Option<i32>,
        structured_output: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let db = self.db.clone();
        let structured_output = structured_output.map(String::from);
        let error_message = error_message.map(String::from);
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            conn.execute(
                "UPDATE execution_log
                 SET finished_at = datetime('now'), exit_code = ?1,
                     structured_output = ?2, error_message = ?3
                 WHERE id = ?4",
                rusqlite::params![exit_code, structured_output, error_message, log_id],
            )
            .context("record_finish failed")?;
            Ok(())
        })
        .await
        .map_err(|e| {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            anyhow::anyhow!("spawn_blocking task failed: {e}")
        })?
    }

    async fn find_by_issue(&self, issue_id: i64) -> Result<Vec<ExecutionLog>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            let mut stmt = conn.prepare(
                "SELECT id, issue_id, state, started_at, finished_at,
                        exit_code, structured_output, error_message
                 FROM execution_log WHERE issue_id = ?1 ORDER BY id",
            )?;
            let logs = stmt
                .query_map([issue_id], |row| {
                    let state_str: String = row.get(2)?;
                    let started_str: String = row.get(3)?;
                    let finished_str: Option<String> = row.get(4)?;

                    Ok(ExecutionLog {
                        id: row.get(0)?,
                        issue_id: row.get(1)?,
                        state: str_to_state(2, &state_str)?,
                        started_at: parse_sqlite_datetime(3, &started_str)?,
                        finished_at: finished_str
                            .as_deref()
                            .map(|s| parse_sqlite_datetime(4, s))
                            .transpose()?,
                        exit_code: row.get(5)?,
                        structured_output: row.get(6)?,
                        error_message: row.get(7)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("find_by_issue query failed")?;
            Ok(logs)
        })
        .await
        .map_err(|e| {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            anyhow::anyhow!("spawn_blocking task failed: {e}")
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::outbound::sqlite_connection::SqliteConnection;
    use crate::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
    use crate::application::port::issue_repository::IssueRepository;
    use crate::domain::issue::Issue;

    fn setup() -> (SqliteExecutionLogRepository, SqliteIssueRepository) {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        (
            SqliteExecutionLogRepository::new(db.clone()),
            SqliteIssueRepository::new(db),
        )
    }

    fn new_issue(n: u64) -> Issue {
        Issue {
            id: 0,
            github_issue_number: n,
            state: State::Idle,
            worktree_path: None,
            ci_fix_count: 0,
            ci_fix_limit_notified: false,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
            body_hash: None,
            feature_name: format!("issue-{n}"),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn record_start_and_find() {
        let (log_repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(1)).await.expect("save");

        let log_id = log_repo
            .record_start(issue_id, State::DesignRunning)
            .await
            .expect("record_start");
        assert!(log_id > 0);

        let logs = log_repo.find_by_issue(issue_id).await.expect("find");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].state, State::DesignRunning);
        assert!(logs[0].finished_at.is_none());
        assert!(logs[0].exit_code.is_none());
    }

    #[tokio::test]
    async fn record_finish_updates_log() {
        let (log_repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(2)).await.expect("save");

        let log_id = log_repo
            .record_start(issue_id, State::DesignRunning)
            .await
            .expect("start");

        log_repo
            .record_finish(log_id, Some(0), Some(r#"{"pr_title":"t"}"#), None)
            .await
            .expect("finish");

        let logs = log_repo.find_by_issue(issue_id).await.expect("find");
        assert_eq!(logs.len(), 1);
        assert!(logs[0].finished_at.is_some());
        assert_eq!(logs[0].exit_code, Some(0));
        assert_eq!(
            logs[0].structured_output.as_deref(),
            Some(r#"{"pr_title":"t"}"#)
        );
    }

    #[tokio::test]
    async fn multiple_logs_per_issue() {
        let (log_repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(3)).await.expect("save");

        log_repo
            .record_start(issue_id, State::DesignRunning)
            .await
            .expect("start 1");
        log_repo
            .record_start(issue_id, State::DesignRunning)
            .await
            .expect("start 2");

        let logs = log_repo.find_by_issue(issue_id).await.expect("find");
        assert_eq!(logs.len(), 2);
    }
}
