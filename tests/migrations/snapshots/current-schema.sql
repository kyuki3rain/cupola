CREATE TABLE execution_log (
                id                INTEGER PRIMARY KEY,
                issue_id          INTEGER NOT NULL REFERENCES issues(id),
                state             TEXT NOT NULL,
                started_at        TEXT NOT NULL DEFAULT (datetime('now')),
                finished_at       TEXT,
                exit_code         INTEGER,
                structured_output TEXT,
                error_message     TEXT
            );
CREATE INDEX idx_execution_log_issue_id
                ON execution_log(issue_id);
CREATE INDEX idx_process_runs_issue_id
                ON process_runs(issue_id);
CREATE TABLE issues (
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
            , last_pr_review_submitted_at TEXT, body_hash TEXT);
CREATE TABLE process_runs (
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