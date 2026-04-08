use anyhow::{Context, Result};

use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::domain::fixing_problem_kind::FixingProblemKind;
use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};

use super::sqlite_connection::SqliteConnection;

pub struct SqliteProcessRunRepository {
    db: SqliteConnection,
}

impl SqliteProcessRunRepository {
    pub fn new(db: SqliteConnection) -> Self {
        Self { db }
    }
}

fn parse_datetime(col_idx: usize, s: &str) -> rusqlite::Result<chrono::DateTime<chrono::Utc>> {
    // Try RFC3339 first (e.g. "2024-01-01T00:00:00Z")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&chrono::Utc));
    }
    // Fall back to SQLite datetime format
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|naive| naive.and_utc())
        .map_err(|_| {
            rusqlite::Error::InvalidColumnType(col_idx, s.to_string(), rusqlite::types::Type::Text)
        })
}

fn row_to_process_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProcessRun> {
    let type_str: String = row.get(2)?;
    let state_str: String = row.get(4)?;
    let started_str: String = row.get(8)?;
    let finished_str: Option<String> = row.get(9)?;
    let causes_str: String = row.get(7)?;

    let type_ = type_str.parse::<ProcessRunType>().map_err(|e| {
        rusqlite::Error::InvalidColumnType(2, e.to_string(), rusqlite::types::Type::Text)
    })?;
    let state = state_str.parse::<ProcessRunState>().map_err(|e| {
        rusqlite::Error::InvalidColumnType(4, e.to_string(), rusqlite::types::Type::Text)
    })?;
    let causes: Vec<FixingProblemKind> = serde_json::from_str(&causes_str).unwrap_or_default();
    let started_at = parse_datetime(8, &started_str)?;
    let finished_at = finished_str
        .as_deref()
        .map(|s| parse_datetime(9, s))
        .transpose()?;

    Ok(ProcessRun {
        id: row.get(0)?,
        issue_id: row.get(1)?,
        type_,
        index: row.get::<_, i64>(3)? as u32,
        state,
        pid: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
        pr_number: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
        causes,
        error_message: row.get(10)?,
        started_at,
        finished_at,
    })
}

impl ProcessRunRepository for SqliteProcessRunRepository {
    async fn save(&self, run: &ProcessRun) -> Result<i64> {
        let db = self.db.clone();
        let issue_id = run.issue_id;
        let type_str = run.type_.to_string();
        let index = run.index as i64;
        let state_str = run.state.to_string();
        let pid = run.pid.map(|p| p as i64);
        let pr_number = run.pr_number.map(|n| n as i64);
        let causes_json = serde_json::to_string(&run.causes).unwrap_or_else(|_| "[]".to_string());
        let error_message = run.error_message.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            conn.execute(
                "INSERT INTO process_runs
                 (issue_id, type, idx, state, pid, pr_number, causes, error_message)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    issue_id,
                    type_str,
                    index,
                    state_str,
                    pid,
                    pr_number,
                    causes_json,
                    error_message
                ],
            )
            .context("process_run save failed")?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn update_pid(&self, run_id: i64, pid: u32) -> Result<()> {
        let db = self.db.clone();
        let pid_i64 = pid as i64;
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            conn.execute(
                "UPDATE process_runs SET pid = ?1 WHERE id = ?2",
                rusqlite::params![pid_i64, run_id],
            )
            .context("update_pid failed")?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn mark_succeeded(&self, run_id: i64, pr_number: Option<u64>) -> Result<()> {
        let db = self.db.clone();
        let pr = pr_number.map(|n| n as i64);
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            conn.execute(
                "UPDATE process_runs
                 SET state = 'succeeded', pr_number = ?1, finished_at = datetime('now')
                 WHERE id = ?2",
                rusqlite::params![pr, run_id],
            )
            .context("mark_succeeded failed")?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn mark_failed(&self, run_id: i64, error_message: Option<String>) -> Result<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            conn.execute(
                "UPDATE process_runs
                 SET state = 'failed', error_message = ?1, finished_at = datetime('now')
                 WHERE id = ?2",
                rusqlite::params![error_message, run_id],
            )
            .context("mark_failed failed")?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn mark_stale(&self, run_id: i64) -> Result<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            conn.execute(
                "UPDATE process_runs
                 SET state = 'stale', pid = NULL, finished_at = datetime('now')
                 WHERE id = ?1",
                rusqlite::params![run_id],
            )
            .context("mark_stale failed")?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn mark_stale_for_issue(&self, issue_id: i64) -> Result<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            conn.execute(
                "UPDATE process_runs
                 SET state = 'stale', pid = NULL, finished_at = datetime('now')
                 WHERE issue_id = ?1 AND state = 'running'",
                rusqlite::params![issue_id],
            )
            .context("mark_stale_for_issue failed")?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn find_latest(
        &self,
        issue_id: i64,
        type_: ProcessRunType,
    ) -> Result<Option<ProcessRun>> {
        let db = self.db.clone();
        let type_str = type_.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let mut stmt = conn.prepare(
                "SELECT id, issue_id, type, idx, state, pid, pr_number, causes,
                        started_at, finished_at, error_message
                 FROM process_runs
                 WHERE issue_id = ?1 AND type = ?2
                 ORDER BY id DESC LIMIT 1",
            )?;
            let mut rows = stmt.query(rusqlite::params![issue_id, type_str])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row_to_process_run(row)?))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn find_by_issue(&self, issue_id: i64) -> Result<Vec<ProcessRun>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let mut stmt = conn.prepare(
                "SELECT id, issue_id, type, idx, state, pid, pr_number, causes,
                        started_at, finished_at, error_message
                 FROM process_runs WHERE issue_id = ?1 ORDER BY id ASC",
            )?;
            let runs = stmt
                .query_map(rusqlite::params![issue_id], row_to_process_run)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("find_by_issue query failed")?;
            Ok(runs)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn count_consecutive_failures(
        &self,
        issue_id: i64,
        type_: ProcessRunType,
    ) -> Result<u32> {
        let db = self.db.clone();
        let type_str = type_.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let mut stmt = conn.prepare(
                "SELECT state FROM process_runs
                 WHERE issue_id = ?1 AND type = ?2
                 ORDER BY id DESC",
            )?;
            let states: Vec<String> = stmt
                .query_map(rusqlite::params![issue_id, type_str], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            let mut count = 0u32;
            for state_str in &states {
                if state_str == "failed" {
                    count += 1;
                } else {
                    break;
                }
            }
            Ok(count)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn find_all_running(&self) -> Result<Vec<ProcessRun>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            let mut stmt = conn.prepare(
                "SELECT id, issue_id, type, idx, state, pid, pr_number, causes,
                        started_at, finished_at, error_message
                 FROM process_runs WHERE state = 'running' ORDER BY id ASC",
            )?;
            let runs = stmt
                .query_map([], row_to_process_run)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("find_all_running query failed")?;
            Ok(runs)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }

    async fn update_state(&self, run_id: i64, state: ProcessRunState) -> Result<()> {
        let db = self.db.clone();
        let state_str = state.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn()
                .lock()
                .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
            conn.execute(
                "UPDATE process_runs SET state = ?1 WHERE id = ?2",
                rusqlite::params![state_str, run_id],
            )
            .context("update_state failed")?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
    use crate::application::port::issue_repository::IssueRepository;
    use crate::domain::issue::Issue;
    use crate::domain::state::State;

    fn setup() -> (SqliteProcessRunRepository, SqliteIssueRepository) {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init");
        (
            SqliteProcessRunRepository::new(db.clone()),
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
            close_finished: false,
            consecutive_failures_epoch: None,
            feature_name: format!("issue-{n}"),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn new_run(issue_id: i64, type_: ProcessRunType) -> ProcessRun {
        ProcessRun::new_running(issue_id, type_, 0, vec![])
    }

    /// T-2.R.1: save returns a positive id
    #[tokio::test]
    async fn save_returns_positive_id() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(1)).await.expect("save issue");
        let run = new_run(issue_id, ProcessRunType::Design);
        let id = repo.save(&run).await.expect("save");
        assert!(id > 0);
    }

    /// T-2.R.2: find_by_issue returns saved runs in insertion order
    #[tokio::test]
    async fn find_by_issue_returns_saved_runs() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(2)).await.expect("save issue");

        let r1 = new_run(issue_id, ProcessRunType::Design);
        let r2 = new_run(issue_id, ProcessRunType::Impl);
        repo.save(&r1).await.expect("save r1");
        repo.save(&r2).await.expect("save r2");

        let runs = repo.find_by_issue(issue_id).await.expect("find");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].type_, ProcessRunType::Design);
        assert_eq!(runs[1].type_, ProcessRunType::Impl);
    }

    /// T-2.R.3: find_latest returns the most recent run for a given type
    #[tokio::test]
    async fn find_latest_returns_most_recent() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(3)).await.expect("save issue");

        let r1 = new_run(issue_id, ProcessRunType::Design);
        let id1 = repo.save(&r1).await.expect("save r1");
        let r2 = new_run(issue_id, ProcessRunType::Design);
        let id2 = repo.save(&r2).await.expect("save r2");

        assert_ne!(id1, id2);

        let latest = repo
            .find_latest(issue_id, ProcessRunType::Design)
            .await
            .expect("find_latest");
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().id, id2);
    }

    /// T-2.R.4: update_pid persists pid
    #[tokio::test]
    async fn update_pid_persists() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(4)).await.expect("save issue");
        let run = new_run(issue_id, ProcessRunType::Init);
        let run_id = repo.save(&run).await.expect("save");

        repo.update_pid(run_id, 12345).await.expect("update_pid");

        let latest = repo
            .find_latest(issue_id, ProcessRunType::Init)
            .await
            .expect("find_latest")
            .expect("exists");
        assert_eq!(latest.pid, Some(12345));
    }

    /// T-2.R.5: mark_succeeded transitions state and sets pr_number
    #[tokio::test]
    async fn mark_succeeded_transitions_state() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(5)).await.expect("save issue");
        let run = new_run(issue_id, ProcessRunType::Design);
        let run_id = repo.save(&run).await.expect("save");

        repo.mark_succeeded(run_id, Some(42))
            .await
            .expect("mark_succeeded");

        let latest = repo
            .find_latest(issue_id, ProcessRunType::Design)
            .await
            .expect("find_latest")
            .expect("exists");
        assert_eq!(latest.state, ProcessRunState::Succeeded);
        assert_eq!(latest.pr_number, Some(42));
        assert!(latest.finished_at.is_some());
    }

    /// T-2.R.6: mark_failed transitions state and sets error_message
    #[tokio::test]
    async fn mark_failed_transitions_state() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(6)).await.expect("save issue");
        let run = new_run(issue_id, ProcessRunType::Impl);
        let run_id = repo.save(&run).await.expect("save");

        repo.mark_failed(run_id, Some("timeout".to_string()))
            .await
            .expect("mark_failed");

        let latest = repo
            .find_latest(issue_id, ProcessRunType::Impl)
            .await
            .expect("find_latest")
            .expect("exists");
        assert_eq!(latest.state, ProcessRunState::Failed);
        assert_eq!(latest.error_message.as_deref(), Some("timeout"));
        assert!(latest.finished_at.is_some());
    }

    /// T-2.R.7: count_consecutive_failures counts tail failures
    #[tokio::test]
    async fn count_consecutive_failures_counts_tail() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(7)).await.expect("save issue");

        // succeeded → failed × 2  → consecutive = 2
        let id1 = repo
            .save(&new_run(issue_id, ProcessRunType::Design))
            .await
            .expect("r1");
        repo.mark_succeeded(id1, None).await.expect("s1");

        let id2 = repo
            .save(&new_run(issue_id, ProcessRunType::Design))
            .await
            .expect("r2");
        repo.mark_failed(id2, None).await.expect("f2");

        let id3 = repo
            .save(&new_run(issue_id, ProcessRunType::Design))
            .await
            .expect("r3");
        repo.mark_failed(id3, None).await.expect("f3");

        let count = repo
            .count_consecutive_failures(issue_id, ProcessRunType::Design)
            .await
            .expect("count");
        assert_eq!(count, 2);
    }

    /// T-2.R.8: mark_stale_for_issue marks running → stale
    #[tokio::test]
    async fn mark_stale_for_issue_marks_running() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(8)).await.expect("save issue");

        let id1 = repo
            .save(&new_run(issue_id, ProcessRunType::Design))
            .await
            .expect("r1");
        // r1 stays running

        let id2 = repo
            .save(&new_run(issue_id, ProcessRunType::Impl))
            .await
            .expect("r2");
        repo.mark_succeeded(id2, None).await.expect("s2");

        repo.mark_stale_for_issue(issue_id)
            .await
            .expect("mark_stale");

        let runs = repo.find_by_issue(issue_id).await.expect("find");
        let r1 = runs.iter().find(|r| r.id == id1).expect("r1");
        let r2 = runs.iter().find(|r| r.id == id2).expect("r2");

        assert_eq!(
            r1.state,
            ProcessRunState::Stale,
            "running should become stale"
        );
        assert_eq!(
            r2.state,
            ProcessRunState::Succeeded,
            "succeeded should stay"
        );
    }

    /// T-2.R.9: find_all_running returns only running records across issues
    #[tokio::test]
    async fn find_all_running_returns_only_running() {
        let (repo, issue_repo) = setup();
        let i1 = issue_repo.save(&new_issue(9)).await.expect("save i1");
        let i2 = issue_repo.save(&new_issue(10)).await.expect("save i2");

        let id1 = repo
            .save(&new_run(i1, ProcessRunType::Design))
            .await
            .expect("r1");
        // id1 stays running
        let id2 = repo
            .save(&new_run(i2, ProcessRunType::Impl))
            .await
            .expect("r2");
        repo.mark_succeeded(id2, None).await.expect("s2");

        let running = repo.find_all_running().await.expect("find");
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].id, id1);
    }
}
