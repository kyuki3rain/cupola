use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::application::port::issue_repository::IssueRepository;
use crate::domain::issue::Issue;
use crate::domain::metadata_update::MetadataUpdates;
use crate::domain::state::State;
use crate::domain::task_weight::TaskWeight;

use super::sqlite_connection::SqliteConnection;
use super::sqlite_helpers::{parse_sqlite_datetime, str_to_state};

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
            let conn = db.conn_lock();
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, feature_name, weight,
                        worktree_path, ci_fix_count, close_finished, consecutive_failures_epoch,
                        created_at, updated_at, last_pr_review_submitted_at, body_hash
                 FROM issues WHERE id = ?1",
            )?;
            let issue = stmt
                .query_row([id], row_to_issue)
                .optional()
                .context("find_by_id query failed")?;
            Ok(issue)
        })
        .await
        .map_err(|e| {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            anyhow::anyhow!("spawn_blocking task failed: {e}")
        })?
    }

    async fn find_by_issue_number(&self, issue_number: u64) -> Result<Option<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || -> Result<Option<Issue>> {
            let conn = db.conn_lock();
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, feature_name, weight,
                        worktree_path, ci_fix_count, close_finished, consecutive_failures_epoch,
                        created_at, updated_at, last_pr_review_submitted_at, body_hash
                 FROM issues WHERE github_issue_number = ?1",
            )?;
            let issue = stmt
                .query_row([issue_number], row_to_issue)
                .optional()
                .context("find_by_issue_number query failed")?;
            Ok(issue)
        })
        .await
        .map_err(|e| {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            anyhow::anyhow!("spawn_blocking task failed: {e}")
        })?
    }

    async fn find_active(&self) -> Result<Vec<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, feature_name, weight,
                        worktree_path, ci_fix_count, close_finished, consecutive_failures_epoch,
                        created_at, updated_at, last_pr_review_submitted_at, body_hash
                 FROM issues WHERE state NOT IN ('completed', 'cancelled')",
            )?;
            let issues = stmt
                .query_map([], row_to_issue)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("find_active query failed")?;
            Ok(issues)
        })
        .await
        .map_err(|e| {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            anyhow::anyhow!("spawn_blocking task failed: {e}")
        })?
    }

    async fn find_all(&self) -> Result<Vec<Issue>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, feature_name, weight,
                        worktree_path, ci_fix_count, close_finished, consecutive_failures_epoch,
                        created_at, updated_at, last_pr_review_submitted_at, body_hash
                 FROM issues",
            )?;
            let issues = stmt
                .query_map([], row_to_issue)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("find_all query failed")?;
            Ok(issues)
        })
        .await
        .map_err(|e| {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            anyhow::anyhow!("spawn_blocking task failed: {e}")
        })?
    }

    async fn save(&self, issue: &Issue) -> Result<i64> {
        let issue = issue.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            conn.execute(
                "INSERT INTO issues (github_issue_number, state, feature_name, weight,
                                     worktree_path, ci_fix_count, close_finished,
                                     consecutive_failures_epoch)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    issue.github_issue_number,
                    issue.state.to_string(),
                    issue.feature_name,
                    task_weight_to_str(issue.weight),
                    issue.worktree_path,
                    issue.ci_fix_count,
                    issue.close_finished as i64,
                    issue
                        .consecutive_failures_epoch
                        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string()),
                ],
            )
            .context("save issue failed")?;
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

    async fn update_state(&self, id: i64, state: State) -> Result<()> {
        let db = self.db.clone();
        let state_str = state.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            conn.execute(
                "UPDATE issues SET state = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![state_str, id],
            )
            .context("update_state failed")?;
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

    async fn update(&self, issue: &Issue) -> Result<()> {
        let issue = issue.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            conn.execute(
                "UPDATE issues SET state = ?1, feature_name = ?2, weight = ?3,
                                   worktree_path = ?4, ci_fix_count = ?5, close_finished = ?6,
                                   consecutive_failures_epoch = ?7, updated_at = datetime('now')
                 WHERE id = ?8",
                rusqlite::params![
                    issue.state.to_string(),
                    issue.feature_name,
                    task_weight_to_str(issue.weight),
                    issue.worktree_path,
                    issue.ci_fix_count,
                    issue.close_finished as i64,
                    issue
                        .consecutive_failures_epoch
                        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string()),
                    issue.id,
                ],
            )
            .context("update issue failed")?;
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

    async fn update_state_and_metadata(
        &self,
        issue_id: i64,
        updates: &MetadataUpdates,
    ) -> Result<()> {
        let updates = updates.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            // Build dynamic SQL update.
            // Push each value first, then derive the ?N placeholder from param_values.len().
            // This eliminates manual index tracking and prevents silent data corruption
            // when fields are added, removed, or reordered.
            let mut set_clauses = vec!["updated_at = datetime('now')".to_string()];
            let mut param_values: Vec<rusqlite::types::Value> = Vec::new();

            if let Some(state) = updates.state {
                param_values.push(rusqlite::types::Value::Text(state.to_string()));
                set_clauses.push(format!("state = ?{}", param_values.len()));
            }
            if let Some(weight) = updates.weight {
                param_values.push(rusqlite::types::Value::Text(
                    task_weight_to_str(weight).to_string(),
                ));
                set_clauses.push(format!("weight = ?{}", param_values.len()));
            }
            if let Some(ci_fix_count) = updates.ci_fix_count {
                param_values.push(rusqlite::types::Value::Integer(ci_fix_count as i64));
                set_clauses.push(format!("ci_fix_count = ?{}", param_values.len()));
            }
            if let Some(close_finished) = updates.close_finished {
                param_values.push(rusqlite::types::Value::Integer(close_finished as i64));
                set_clauses.push(format!("close_finished = ?{}", param_values.len()));
            }
            if let Some(ref feature_name) = updates.feature_name {
                param_values.push(rusqlite::types::Value::Text(feature_name.clone()));
                set_clauses.push(format!("feature_name = ?{}", param_values.len()));
            }
            if let Some(epoch) = updates.consecutive_failures_epoch {
                let v = epoch
                    .map(|dt| {
                        rusqlite::types::Value::Text(dt.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string())
                    })
                    .unwrap_or(rusqlite::types::Value::Null);
                param_values.push(v);
                set_clauses.push(format!(
                    "consecutive_failures_epoch = ?{}",
                    param_values.len()
                ));
            }
            if let Some(ref worktree_path) = updates.worktree_path {
                let v = worktree_path
                    .as_ref()
                    .map(|p| rusqlite::types::Value::Text(p.clone()))
                    .unwrap_or(rusqlite::types::Value::Null);
                param_values.push(v);
                set_clauses.push(format!("worktree_path = ?{}", param_values.len()));
            }
            if let Some(ts) = updates.last_pr_review_submitted_at {
                param_values.push(rusqlite::types::Value::Text(
                    ts.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string(),
                ));
                set_clauses.push(format!(
                    "last_pr_review_submitted_at = ?{}",
                    param_values.len()
                ));
            }
            if let Some(ref body_hash) = updates.body_hash {
                let v = body_hash
                    .as_ref()
                    .map(|h| rusqlite::types::Value::Text(h.clone()))
                    .unwrap_or(rusqlite::types::Value::Null);
                param_values.push(v);
                set_clauses.push(format!("body_hash = ?{}", param_values.len()));
            }

            // WHERE id comes after all SET params; index is len+1 before pushing.
            let where_idx = param_values.len() + 1;
            let sql = format!(
                "UPDATE issues SET {} WHERE id = ?{}",
                set_clauses.join(", "),
                where_idx
            );
            param_values.push(rusqlite::types::Value::Integer(issue_id));

            let params: Vec<&dyn rusqlite::ToSql> = param_values
                .iter()
                .map(|v| v as &dyn rusqlite::ToSql)
                .collect();

            conn.execute(&sql, params.as_slice())
                .context("update_state_and_metadata failed")?;
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

    async fn find_by_state(&self, state: State) -> Result<Vec<Issue>> {
        let db = self.db.clone();
        let state_str = state.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn_lock();
            let mut stmt = conn.prepare(
                "SELECT id, github_issue_number, state, feature_name, weight,
                        worktree_path, ci_fix_count, close_finished, consecutive_failures_epoch,
                        created_at, updated_at, last_pr_review_submitted_at, body_hash
                 FROM issues WHERE state = ?1",
            )?;
            let issues = stmt
                .query_map([state_str], row_to_issue)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("find_by_state query failed")?;
            Ok(issues)
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
    let weight_str: String = row.get(4)?;
    let created_str: String = row.get(9)?;
    let updated_str: String = row.get(10)?;

    let github_issue_number: u64 = row.get(1)?;

    let consecutive_failures_epoch: Option<String> = row.get(8)?;
    let consecutive_failures_epoch = consecutive_failures_epoch
        .map(|s| parse_rfc3339_datetime(8, &s))
        .transpose()?;

    let last_pr_review_submitted_at: Option<String> = row.get(11)?;
    let last_pr_review_submitted_at = last_pr_review_submitted_at
        .map(|s| parse_rfc3339_datetime(11, &s))
        .transpose()?;

    Ok(Issue {
        id: row.get(0)?,
        github_issue_number,
        state: str_to_state(2, &state_str)?,
        feature_name: row
            .get::<_, Option<String>>(3)?
            .unwrap_or_else(|| format!("issue-{github_issue_number}")),
        weight: str_to_task_weight(4, &weight_str)?,
        worktree_path: row.get(5)?,
        ci_fix_count: {
            let v: i64 = row.get(6)?;
            v as u32
        },
        close_finished: {
            let v: i64 = row.get(7)?;
            v != 0
        },
        consecutive_failures_epoch,
        last_pr_review_submitted_at,
        body_hash: row.get(12)?,
        created_at: parse_sqlite_datetime(9, &created_str)?,
        updated_at: parse_sqlite_datetime(10, &updated_str)?,
    })
}

fn parse_rfc3339_datetime(col_idx: usize, s: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Try parsing as sqlite datetime format
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .map(|naive| naive.and_utc())
                .map_err(|_| ())
        })
        .map_err(|_| {
            rusqlite::Error::InvalidColumnType(col_idx, s.to_owned(), rusqlite::types::Type::Text)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::outbound::sqlite_connection::SqliteConnection;
    use chrono::TimeZone;

    fn setup() -> (SqliteConnection, SqliteIssueRepository) {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("init schema");
        let repo = SqliteIssueRepository::new(db.clone());
        (db, repo)
    }

    fn new_issue(issue_number: u64) -> Issue {
        Issue::new(issue_number, format!("issue-{issue_number}"))
    }

    /// T-2.IR.1: migration idempotency
    #[test]
    fn migration_is_idempotent() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("first init");
        db.init_schema().expect("second init should succeed");
    }

    /// T-2.IR.2: find_active excludes Completed and Cancelled
    #[tokio::test]
    async fn find_active_excludes_terminal() {
        let (_db, repo) = setup();
        let id1 = repo.save(&new_issue(1)).await.expect("save");
        let id2 = repo.save(&new_issue(2)).await.expect("save");
        let id3 = repo.save(&new_issue(3)).await.expect("save");

        repo.update_state(id1, State::DesignRunning)
            .await
            .expect("update");
        repo.update_state(id2, State::Completed)
            .await
            .expect("update");
        repo.update_state(id3, State::Cancelled)
            .await
            .expect("update");

        let active = repo.find_active().await.expect("find_active");
        let numbers: Vec<u64> = active.iter().map(|i| i.github_issue_number).collect();
        assert!(numbers.contains(&1));
        assert!(!numbers.contains(&2));
        assert!(!numbers.contains(&3));
    }

    /// T-2.IR.3: find_all includes Completed and Cancelled
    #[tokio::test]
    async fn find_all_includes_terminal() {
        let (_db, repo) = setup();
        let id1 = repo.save(&new_issue(1)).await.expect("save");
        let id2 = repo.save(&new_issue(2)).await.expect("save");

        repo.update_state(id1, State::Completed)
            .await
            .expect("update");
        repo.update_state(id2, State::Cancelled)
            .await
            .expect("update");

        let all = repo.find_all().await.expect("find_all");
        assert_eq!(all.len(), 2);
    }

    /// T-2.IR.4: update_state_and_metadata writes only non-None fields
    #[tokio::test]
    async fn update_state_and_metadata_writes_only_some_fields() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(10)).await.expect("save");

        let updates = MetadataUpdates {
            state: Some(State::DesignRunning),
            ci_fix_count: Some(3),
            ..Default::default()
        };
        repo.update_state_and_metadata(id, &updates)
            .await
            .expect("update");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.state, State::DesignRunning);
        assert_eq!(found.ci_fix_count, 3);
        // weight was not updated
        assert_eq!(found.weight, TaskWeight::Medium);
    }

    /// T-2.IR.5: consecutive_failures_epoch roundtrips
    #[tokio::test]
    async fn consecutive_failures_epoch_roundtrip() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(20)).await.expect("save");

        // Set epoch
        let epoch = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();
        let updates = MetadataUpdates {
            consecutive_failures_epoch: Some(Some(epoch)),
            ..Default::default()
        };
        repo.update_state_and_metadata(id, &updates)
            .await
            .expect("update");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert!(found.consecutive_failures_epoch.is_some());
        let loaded = found.consecutive_failures_epoch.unwrap();
        // Compare at second precision
        assert_eq!(
            loaded.format("%Y-%m-%dT%H:%M:%S").to_string(),
            epoch.format("%Y-%m-%dT%H:%M:%S").to_string()
        );

        // Set to None
        let updates_none = MetadataUpdates {
            consecutive_failures_epoch: Some(None),
            ..Default::default()
        };
        repo.update_state_and_metadata(id, &updates_none)
            .await
            .expect("update");
        let found2 = repo.find_by_id(id).await.expect("find").expect("exists");
        assert!(found2.consecutive_failures_epoch.is_none());
    }

    /// T-2.IR.6: close_finished roundtrips bool
    #[tokio::test]
    async fn close_finished_roundtrip() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(30)).await.expect("save");

        // Default false
        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert!(!found.close_finished);

        // Set to true
        let updates = MetadataUpdates {
            close_finished: Some(true),
            ..Default::default()
        };
        repo.update_state_and_metadata(id, &updates)
            .await
            .expect("update");
        let found2 = repo.find_by_id(id).await.expect("find").expect("exists");
        assert!(found2.close_finished);
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
        assert!(!found.close_finished);
        assert!(found.consecutive_failures_epoch.is_none());
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
    async fn update_state() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(50)).await.expect("save");

        repo.update_state(id, State::InitializeRunning)
            .await
            .expect("update_state");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.state, State::InitializeRunning);
    }

    #[tokio::test]
    async fn update_full_issue() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(60)).await.expect("save");

        let mut issue = repo.find_by_id(id).await.expect("find").expect("exists");
        issue.state = State::DesignRunning;
        issue.worktree_path = Some("/tmp/wt".to_string());
        issue.ci_fix_count = 2;
        issue.close_finished = true;

        repo.update(&issue).await.expect("update");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.state, State::DesignRunning);
        assert_eq!(found.worktree_path.as_deref(), Some("/tmp/wt"));
        assert_eq!(found.ci_fix_count, 2);
        assert!(found.close_finished);
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
    }

    #[test]
    fn state_roundtrip_includes_initialize_running() {
        for s in crate::domain::state::State::all() {
            let displayed = s.to_string();
            assert_eq!(str_to_state(0, &displayed).unwrap(), s);
        }
    }

    /// T-1.2.bh.1: body_hash を保存して find_by_id で正しく取得できること
    #[tokio::test]
    async fn body_hash_roundtrip() {
        let (_db, repo) = setup();
        let id = repo.save(&new_issue(300)).await.expect("save");

        // 初期値は None
        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert!(found.body_hash.is_none());

        // body_hash を設定
        let hash = "abc123def456".to_string();
        let updates = MetadataUpdates {
            body_hash: Some(Some(hash.clone())),
            ..Default::default()
        };
        repo.update_state_and_metadata(id, &updates)
            .await
            .expect("update");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert_eq!(found.body_hash.as_deref(), Some(hash.as_str()));

        // body_hash を NULL クリア
        let updates_clear = MetadataUpdates {
            body_hash: Some(None),
            ..Default::default()
        };
        repo.update_state_and_metadata(id, &updates_clear)
            .await
            .expect("update");

        let found = repo.find_by_id(id).await.expect("find").expect("exists");
        assert!(found.body_hash.is_none());
    }

    /// T-1.2.bh.2: body_hash マイグレーション冪等性（2 回実行しても失敗しない）
    #[test]
    fn body_hash_migration_is_idempotent() {
        let db = SqliteConnection::open_in_memory().expect("open");
        db.init_schema().expect("first init");
        db.init_schema().expect("second init should succeed");

        // body_hash カラムが存在することを確認
        let conn = db.conn().lock().expect("mutex");
        let has_col: bool = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('issues') WHERE name = 'body_hash'",
                [],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .expect("pragma query");
        assert!(has_col, "body_hash column should exist after migration");
    }

    /// T-2.IR.7: update_state_and_metadata updates only the target row, not adjacent rows.
    ///
    /// This guards against dynamic SQL binding fragility: if `?N` placeholder indices
    /// are computed incorrectly, the WHERE clause may bind to the wrong value and
    /// silently update a different row.
    #[tokio::test]
    async fn update_state_and_metadata_does_not_corrupt_adjacent_rows() {
        let (_db, repo) = setup();
        let id1 = repo.save(&new_issue(200)).await.expect("save 1");
        let id2 = repo.save(&new_issue(201)).await.expect("save 2");
        let id3 = repo.save(&new_issue(202)).await.expect("save 3");

        // Update all available fields on id2 only
        let updates = MetadataUpdates {
            state: Some(State::DesignRunning),
            weight: Some(TaskWeight::Heavy),
            ci_fix_count: Some(5),
            close_finished: Some(true),
            feature_name: Some("updated-feature".to_string()),
            consecutive_failures_epoch: Some(Some(
                chrono::Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            )),
            worktree_path: Some(Some("/tmp/wt".to_string())),
            last_pr_review_submitted_at: None,
            body_hash: None,
        };
        repo.update_state_and_metadata(id2, &updates)
            .await
            .expect("update");

        // id2 should be updated
        let found2 = repo.find_by_id(id2).await.expect("find").expect("exists");
        assert_eq!(found2.state, State::DesignRunning);
        assert_eq!(found2.weight, TaskWeight::Heavy);
        assert_eq!(found2.ci_fix_count, 5);
        assert!(found2.close_finished);
        assert_eq!(found2.feature_name, "updated-feature");
        assert!(found2.consecutive_failures_epoch.is_some());
        assert_eq!(found2.worktree_path.as_deref(), Some("/tmp/wt"));

        // id1 and id3 must be unchanged (default values)
        let found1 = repo.find_by_id(id1).await.expect("find").expect("exists");
        assert_eq!(found1.state, State::Idle, "id1 state must be unchanged");
        assert_eq!(found1.ci_fix_count, 0, "id1 ci_fix_count must be unchanged");
        assert!(
            !found1.close_finished,
            "id1 close_finished must be unchanged"
        );

        let found3 = repo.find_by_id(id3).await.expect("find").expect("exists");
        assert_eq!(found3.state, State::Idle, "id3 state must be unchanged");
        assert_eq!(found3.ci_fix_count, 0, "id3 ci_fix_count must be unchanged");
    }
}
