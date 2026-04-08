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

    pub fn init_schema(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS issues (
                id                          INTEGER PRIMARY KEY,
                github_issue_number         INTEGER UNIQUE NOT NULL,
                state                       TEXT NOT NULL DEFAULT 'idle',
                feature_name                TEXT,
                weight                      TEXT NOT NULL DEFAULT 'medium',
                worktree_path               TEXT,
                ci_fix_count                INTEGER NOT NULL DEFAULT 0,
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
        Self::run_add_column_migration(&conn, "close_finished INTEGER NOT NULL DEFAULT 0")?;
        Self::run_add_column_migration(&conn, "consecutive_failures_epoch TEXT")?;

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
