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
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to acquire database lock"))?;
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
                feature_name        TEXT,
                model               TEXT
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
        Self::run_add_column_migration(&conn, "feature_name TEXT")?;

        // Migration: add model column for existing databases
        Self::run_add_column_migration(&conn, "model TEXT")?;

        // Migration: add fixing_causes column for existing databases
        Self::run_add_column_migration(&conn, "fixing_causes TEXT NOT NULL DEFAULT '[]'")?;

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
    fn migration_adds_model_column_to_existing_db() {
        let db = SqliteConnection::open_in_memory().expect("should open");
        let conn = db.conn().lock().expect("mutex");

        // Create a legacy issues table without model column
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

        // Verify model column exists and existing record has NULL model
        let conn = db.conn().lock().expect("mutex");
        let model: Option<String> = conn
            .query_row(
                "SELECT model FROM issues WHERE github_issue_number = 1",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert!(model.is_none(), "existing record should have NULL model");
    }

    #[test]
    fn migration_model_column_is_idempotent() {
        let db = SqliteConnection::open_in_memory().expect("should open");
        // Running init_schema twice should not fail even if model column already exists
        db.init_schema().expect("first init");
        db.init_schema()
            .expect("second init should succeed (idempotent)");
    }
}
