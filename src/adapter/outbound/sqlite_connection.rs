use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::application::port::db_initializer::DbInitializer;

#[derive(Clone)]
pub struct SqliteConnection {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteConnection {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open SQLite at {}", path.display()))?;
        Self::init_pragmas(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory SQLite")?;
        Self::init_pragmas(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn init_pragmas(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;
             PRAGMA journal_size_limit = 67108864;",
        )
        .context("failed to set SQLite pragmas")?;
        Ok(())
    }

    /// Mutexロックを取得して返す。
    ///
    /// # Panics
    /// Mutexが毒化している場合（前スレッドがクリティカルセクション内でパニック
    /// した場合）にパニックする。ロック毒化は回復不可能な致命的状態であり、
    /// 通常のエラーとして扱ってはならない。
    pub fn conn_lock(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        // Mutex毒化（PoisonError）は前スレッドがクリティカルセクション内でパニックした
        // 場合にのみ発生する回復不可能な致命的状態であるため、即座にパニックする。
        self.conn
            .lock()
            .unwrap_or_else(|e| panic!("database Mutex poisoned: {e}"))
    }

    pub fn init_schema(&self) -> Result<()> {
        let conn = self.conn_lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS issues (
                id                          INTEGER PRIMARY KEY,
                github_issue_number         INTEGER UNIQUE NOT NULL,
                state                       TEXT NOT NULL DEFAULT 'idle',
                feature_name                TEXT,
                weight                      TEXT NOT NULL DEFAULT 'medium',
                worktree_path               TEXT,
                ci_fix_count                INTEGER NOT NULL DEFAULT 0,
                ci_fix_limit_notified       INTEGER NOT NULL DEFAULT 0,
                close_finished              INTEGER NOT NULL DEFAULT 0,
                consecutive_failures_epoch  TEXT,
                created_at                  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at                  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS process_runs (
                id           INTEGER PRIMARY KEY,
                issue_id     INTEGER NOT NULL REFERENCES issues(id),
                type         TEXT NOT NULL,
                idx          INTEGER NOT NULL DEFAULT 0,
                state        TEXT NOT NULL DEFAULT 'running',
                pid          INTEGER,
                pr_number    INTEGER,
                causes       TEXT NOT NULL DEFAULT '[]',
                error_message TEXT,
                started_at   TEXT NOT NULL DEFAULT (datetime('now')),
                finished_at  TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_process_runs_issue_id
                ON process_runs(issue_id);

            CREATE TABLE IF NOT EXISTS execution_log (
                id                INTEGER PRIMARY KEY,
                issue_id          INTEGER NOT NULL REFERENCES issues(id),
                state             TEXT NOT NULL,
                started_at        TEXT NOT NULL DEFAULT (datetime('now')),
                finished_at       TEXT,
                exit_code         INTEGER,
                structured_output TEXT,
                error_message     TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_execution_log_issue_id
                ON execution_log(issue_id);",
        )
        .context("failed to initialize SQLite schema")?;

        // Migrations for existing databases
        // Add new columns to issues table if they don't exist
        Self::run_add_column_migration(&conn, "feature_name TEXT")?;
        Self::run_add_column_migration(&conn, "weight TEXT NOT NULL DEFAULT 'medium'")?;
        Self::run_add_column_migration(&conn, "ci_fix_count INTEGER NOT NULL DEFAULT 0")?;
        Self::run_add_column_migration(&conn, "ci_fix_limit_notified INTEGER NOT NULL DEFAULT 0")?;
        Self::run_add_column_migration(&conn, "close_finished INTEGER NOT NULL DEFAULT 0")?;
        Self::run_add_column_migration(&conn, "consecutive_failures_epoch TEXT")?;
        Self::run_add_column_migration(&conn, "last_pr_review_submitted_at TEXT")?;

        // Migration: rename 'initialized' state to 'initialize_running'
        conn.execute_batch(
            "UPDATE issues SET state = 'initialize_running' WHERE state = 'initialized';",
        )
        .context("failed to migrate initialized -> initialize_running")?;

        // Migration: backfill NULL feature_name
        conn.execute_batch(
            "UPDATE issues SET feature_name = 'issue-' || github_issue_number WHERE feature_name IS NULL;"
        ).context("failed to backfill feature_name")?;

        Ok(())
    }

    /// `ALTER TABLE issues ADD COLUMN <column_def>` を実行し、
    /// "duplicate column name" エラーのみ無視してそれ以外は伝播する。
    fn run_add_column_migration(conn: &Connection, column_def: &str) -> Result<()> {
        let sql = format!("ALTER TABLE issues ADD COLUMN {column_def};");
        match conn.execute_batch(&sql) {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("duplicate column name") {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(e).context(format!(
                        "failed to add column '{column_def}' to issues table"
                    )))
                }
            }
        }
    }

    pub fn conn(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
    }
}

impl DbInitializer for SqliteConnection {
    fn init_schema(&self) -> Result<()> {
        SqliteConnection::init_schema(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Task 1.2: conn_lock() が健全な Mutex に対して正常に動作することを検証する
    #[test]
    fn conn_lock_succeeds_on_healthy_mutex() {
        let db = SqliteConnection::open_in_memory().expect("should open");
        // 毒化していない場合は MutexGuard が返される（パニックしない）
        let _guard = db.conn_lock();
    }

    /// Task 1.2: Mutex が毒化した場合に conn_lock() がパニックすることを検証する
    #[test]
    #[should_panic(expected = "poisoned")]
    fn conn_lock_panics_on_poisoned_mutex() {
        let db = SqliteConnection::open_in_memory().expect("should open");
        let arc = db.conn().clone();
        // 別スレッドでロックを保持したままパニックし、Mutex を毒化する
        let _ = std::thread::spawn(move || {
            let _guard = arc.lock().unwrap();
            panic!("intentionally poison the mutex");
        })
        .join();
        // Mutex が毒化しているため conn_lock() はパニックする
        drop(db.conn_lock());
    }

    async fn conn_lock_via_spawn_blocking(db: SqliteConnection) {
        tokio::task::spawn_blocking(move || {
            let _guard = db.conn_lock();
        })
        .await
        .expect("spawn_blocking task should not panic");
    }

    /// Task 1.2: 実運用に近い spawn_blocking 経由でも Mutex 毒化が async 呼び出し側でパニックになることを検証する
    #[tokio::test]
    #[should_panic(expected = "spawn_blocking task should not panic")]
    async fn conn_lock_panics_on_poisoned_mutex_via_spawn_blocking() {
        let db = SqliteConnection::open_in_memory().expect("should open");
        let arc = db.conn().clone();
        // 別スレッドでロックを保持したままパニックし、Mutex を毒化する
        let _ = std::thread::spawn(move || {
            let _guard = arc.lock().unwrap();
            panic!("intentionally poison the mutex");
        })
        .join();

        // spawn_blocking 内の panic は JoinError になるため、await 側で unwrap/expect すると panic になる
        conn_lock_via_spawn_blocking(db).await;
    }

    #[test]
    fn open_in_memory_and_init_schema() {
        let db = SqliteConnection::open_in_memory().expect("should open");
        db.init_schema().expect("should init schema");

        // Verify tables exist
        let conn = db.conn().lock().expect("mutex");
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='issues'",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(count, 1);

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='execution_log'",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(count, 1);
    }

    #[test]
    fn init_schema_is_idempotent() {
        let db = SqliteConnection::open_in_memory().expect("should open");
        db.init_schema().expect("first init");
        db.init_schema().expect("second init should succeed");
    }

    #[test]
    fn migration_adds_weight_column_to_existing_db() {
        let db = SqliteConnection::open_in_memory().expect("should open");
        let conn = db.conn().lock().expect("mutex");

        // Create a legacy issues table without weight column
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS issues (
                id                  INTEGER PRIMARY KEY,
                github_issue_number INTEGER UNIQUE NOT NULL,
                state               TEXT NOT NULL DEFAULT 'idle',
                design_pr_number    INTEGER,
                impl_pr_number      INTEGER,
                worktree_path       TEXT,
                retry_count         INTEGER NOT NULL DEFAULT 0,
                current_pid         INTEGER,
                created_at          TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
                error_message       TEXT,
                feature_name        TEXT
            );",
        )
        .expect("create legacy table");

        // Insert existing row before migration
        conn.execute(
            "INSERT INTO issues (github_issue_number, state) VALUES (1, 'idle')",
            [],
        )
        .expect("insert");

        drop(conn);

        // Run migration
        db.init_schema().expect("migration should succeed");

        // Verify weight column exists and existing record has default value 'medium'
        let conn = db.conn().lock().expect("mutex");
        let weight: String = conn
            .query_row(
                "SELECT weight FROM issues WHERE github_issue_number = 1",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(
            weight, "medium",
            "existing record should have default weight 'medium'"
        );
    }

    #[test]
    fn migration_weight_column_is_idempotent() {
        let db = SqliteConnection::open_in_memory().expect("should open");
        // Running init_schema twice should not fail even if weight column already exists
        db.init_schema().expect("first init");
        db.init_schema()
            .expect("second init should succeed (idempotent)");
    }
}
