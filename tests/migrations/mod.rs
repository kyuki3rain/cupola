#![allow(clippy::expect_used)]

use cupola::adapter::outbound::sqlite_connection::SqliteConnection;

fn load_fixture(name: &str) -> (SqliteConnection, tempfile::TempDir) {
    let src = format!("tests/migrations/fixtures/{name}.db");
    let dir = tempfile::tempdir().expect("create temp dir");
    let dst = dir.path().join("fixture.db");
    std::fs::copy(&src, &dst).expect("copy fixture");
    let conn = SqliteConnection::open(&dst).expect("open fixture");
    (conn, dir)
}

fn dump_schema(conn: &SqliteConnection) -> String {
    let guard = conn.conn().lock().expect("lock");
    let mut stmt = guard
        .prepare(
            "SELECT sql FROM sqlite_master \
             WHERE type IN ('table', 'index') AND sql IS NOT NULL \
             ORDER BY name",
        )
        .expect("prepare dump_schema");
    let rows: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query dump_schema")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect dump_schema rows");
    if rows.is_empty() {
        String::new()
    } else {
        rows.join(";\n") + ";"
    }
}

fn normalize_schema(s: &str) -> String {
    s.lines()
        .map(|l| l.trim_end().trim_start_matches('\r'))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn migrate_v0001_is_idempotent() {
    let (conn, _dir) = load_fixture("v0001-initial");

    conn.init_schema().expect("first init_schema");
    conn.init_schema()
        .expect("second init_schema (idempotency check)");

    let guard = conn.conn().lock().expect("lock");

    let state: String = guard
        .query_row(
            "SELECT state FROM issues WHERE github_issue_number = 1",
            [],
            |row| row.get(0),
        )
        .expect("query issue 1");
    assert_eq!(state, "idle", "issue 1 state should remain 'idle'");

    let state: String = guard
        .query_row(
            "SELECT state FROM issues WHERE github_issue_number = 2",
            [],
            |row| row.get(0),
        )
        .expect("query issue 2");
    assert_eq!(
        state, "design_running",
        "issue 2 state should remain 'design_running'"
    );

    let val: Option<String> = guard
        .query_row(
            "SELECT last_pr_review_submitted_at FROM issues WHERE github_issue_number = 1",
            [],
            |row| row.get(0),
        )
        .expect("query last_pr_review_submitted_at");
    assert!(
        val.is_none(),
        "last_pr_review_submitted_at should be NULL after migration"
    );
}

#[test]
fn fixture_reaches_current_schema() {
    let (conn, _dir) = load_fixture("v0001-initial");
    conn.init_schema().expect("init_schema");

    let expected_raw = std::fs::read_to_string("tests/migrations/snapshots/current-schema.sql")
        .expect("read current-schema.sql");

    let actual = dump_schema(&conn);
    let expected = normalize_schema(&expected_raw);
    let actual_norm = normalize_schema(&actual);

    assert_eq!(
        actual_norm, expected,
        "schema mismatch after migration from v0001-initial"
    );
}

#[test]
#[ignore]
fn generate_v0001_fixture() {
    use rusqlite::Connection;
    std::fs::create_dir_all("tests/migrations/fixtures").expect("create fixtures dir");
    let path = "tests/migrations/fixtures/v0001-initial.db";
    if std::path::Path::new(path).exists() {
        std::fs::remove_file(path).expect("remove existing fixture");
    }
    let conn = Connection::open(path).expect("open");
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;",
    )
    .expect("pragmas");
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
    .expect("create tables");
    conn.execute_batch(
        "INSERT INTO issues (github_issue_number, state, feature_name, weight, created_at, updated_at)
         VALUES
             (1, 'idle',           'issue-1', 'medium', datetime('now'), datetime('now')),
             (2, 'design_running', 'issue-2', 'heavy',  datetime('now'), datetime('now'));",
    )
    .expect("insert rows");
    println!("Generated: {path}");
}

#[test]
#[ignore]
fn generate_current_schema_snapshot() {
    std::fs::create_dir_all("tests/migrations/snapshots").expect("create snapshots dir");
    let db = SqliteConnection::open_in_memory().expect("open in-memory");
    db.init_schema().expect("init_schema");
    let schema = dump_schema(&db);
    std::fs::write("tests/migrations/snapshots/current-schema.sql", &schema)
        .expect("write current-schema.sql");
    println!("Generated: tests/migrations/snapshots/current-schema.sql");
    println!("{schema}");
}
