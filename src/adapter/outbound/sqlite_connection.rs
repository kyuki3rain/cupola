use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::Connection;

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
        let conn = self.conn.lock().expect("mutex poisoned");
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
            );

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

        // Migration: add feature_name column for existing databases
        let _ = conn.execute_batch("ALTER TABLE issues ADD COLUMN feature_name TEXT;");

        Ok(())
    }

    pub fn conn(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
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
}
