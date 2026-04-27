# 設計書: DB マイグレーション回帰テストの実装

## Overview

### Goals

- `init_schema()` の idempotency と後方互換性を、fixture ベースの単体テストで継続的に証明する
- `SqliteConnection::dump_schema()` ヘルパーを `#[cfg(test)]` で追加し、スキーマ比較テストを可能にする
- `tests/migrations/` テスト基盤（fixture / snapshots）を整備し、`cargo test` CI に組み込む

### Non-Goals

- E2E テスト（GitHub / Claude Code を使うフロー）の追加
- `association_guard` テストや fix-count-limit テストの追加（別 issue 管理）
- GitHub/process 障害シミュレーション
- fixture の自動生成ツール化（手動運用で十分）

## Requirements Traceability

| Requirement | Summary | 変更対象コンポーネント |
|-------------|---------|----------------------|
| 1.1〜1.4 | `dump_schema()` ヘルパー追加 | `src/adapter/outbound/sqlite_connection.rs` |
| 2.1〜2.5 | テスト基盤整備 | `tests/migrations/` (新規), `Cargo.toml` |
| 3.1〜3.5 | `migrate_v0001_is_idempotent` テスト | `tests/migrations/mod.rs` |
| 4.1〜4.4 | `fixture_reaches_current_schema` テスト | `tests/migrations/mod.rs` |
| 5.1〜5.3 | `Cargo.toml` `[[test]]` 登録 | `Cargo.toml` |
| 6.1〜6.2 | 運用ドキュメント更新 | `CHANGELOG.md` |

## Architecture

### 既存アーキテクチャの分析

`SqliteConnection` は `src/adapter/outbound/sqlite_connection.rs` に配置されている adapter/outbound 層のコンポーネントである。`init_schema()` は `Arc<Mutex<Connection>>` を用いて idempotent にテーブル作成とマイグレーションを実行する。

現行の `init_schema()` が適用するマイグレーション:
1. `CREATE TABLE IF NOT EXISTS issues (...)` — id〜updated_at の 12 カラム
2. `ALTER TABLE issues ADD COLUMN feature_name TEXT` — 既存 DB 向け（現行 CREATE TABLE には含まれるため通常は no-op）
3. 同様に weight / ci_fix_count / ci_fix_limit_notified / close_finished / consecutive_failures_epoch も no-op
4. `ALTER TABLE issues ADD COLUMN last_pr_review_submitted_at TEXT` — 追加カラム
5. `ALTER TABLE issues ADD COLUMN body_hash TEXT` — 追加カラム
6. `UPDATE issues SET state = 'initialize_running' WHERE state = 'initialized'` — state rename
7. `UPDATE issues SET feature_name = 'issue-...' WHERE feature_name IS NULL` — feature_name backfill

`v0001-initial.db` fixture は項目 4・5 が適用される前のスキーマを模擬する。fixture を開いて `init_schema()` を呼ぶと 4・5 が追加され、現行スキーマと一致する。

### コンポーネント構成

```
src/adapter/outbound/sqlite_connection.rs
  └── #[cfg(test)] pub fn dump_schema(&self) -> String   ← 追加

tests/
└── migrations/
    ├── mod.rs                          ← テストエントリポイント (新規)
    ├── fixtures/
    │   ├── v0001-initial.db            ← バイナリ fixture (新規)
    │   └── v0001-initial.README.md     ← fixture 説明 (新規)
    └── snapshots/
        └── current-schema.sql          ← スキーマスナップショット (新規)

Cargo.toml                              ← [[test]] セクション追加
CHANGELOG.md                            ← [Unreleased] セクション更新
```

## Components and Interfaces

### `SqliteConnection::dump_schema()`

| Field | Detail |
|-------|--------|
| 配置 | `src/adapter/outbound/sqlite_connection.rs` の `#[cfg(test)]` impl ブロック |
| Intent | `sqlite_master` からスキーマ定義を取得して文字列で返す |
| Requirements | 1.1〜1.4 |

**実装仕様**

```rust
#[cfg(test)]
pub fn dump_schema(&self) -> String {
    let conn = self.conn_lock();
    let mut stmt = conn
        .prepare(
            "SELECT sql FROM sqlite_master \
             WHERE type IN ('table', 'index') AND sql IS NOT NULL \
             ORDER BY name",
        )
        .expect("prepare dump_schema");
    let rows: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query dump_schema")
        .filter_map(Result::ok)
        .collect();
    if rows.is_empty() {
        String::new()
    } else {
        rows.join(";\n") + ";"
    }
}
```

**補足**:
- `sqlite_master` の `sql` カラムには SQLite が管理する DDL テキストが格納される
- `ALTER TABLE ADD COLUMN` を実行すると SQLite は `sqlite_master.sql` を更新し、追加カラムが反映される
- `ORDER BY name` でテーブル名・インデックス名のアルファベット順に取得することで、環境間での順序差異を排除する

---

### `tests/migrations/mod.rs`

#### ヘルパー関数

**`load_fixture(name: &str) -> (SqliteConnection, tempfile::TempDir)`**

```rust
fn load_fixture(name: &str) -> (SqliteConnection, tempfile::TempDir) {
    let src = format!("tests/migrations/fixtures/{name}.db");
    let dir = tempfile::tempdir().expect("create temp dir");
    let dst = dir.path().join("fixture.db");
    std::fs::copy(&src, &dst).expect("copy fixture");
    let conn = SqliteConnection::open(&dst).expect("open fixture");
    (conn, dir)
}
```

`TempDir` を返値に含めることで、テスト終了まで一時ディレクトリ（および WAL ファイル）が確実に保持される。呼び出し元は `let (_conn, _dir) = load_fixture(...)` のように `_dir` を束縛して生存期間を管理する。

**`normalize_schema(s: &str) -> String`**

```rust
fn normalize_schema(s: &str) -> String {
    s.lines()
        .map(|l| l.trim_end().trim_start_matches('\r'))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
```

CRLF → LF 変換・末尾空白除去・空行除去を行う。SQLite バージョン差による軽微なフォーマット差異を吸収する。

#### テスト: `migrate_v0001_is_idempotent`

```rust
#[test]
fn migrate_v0001_is_idempotent() {
    let (conn, _dir) = load_fixture("v0001-initial");

    conn.init_schema().expect("first init_schema");
    conn.init_schema().expect("second init_schema (idempotency check)");

    let guard = conn.conn().lock().expect("lock");

    // 既存行の保全
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
    assert_eq!(state, "design_running", "issue 2 state should remain 'design_running'");

    // 新カラムのデフォルト値（NULL）検証
    let val: Option<String> = guard
        .query_row(
            "SELECT last_pr_review_submitted_at FROM issues WHERE github_issue_number = 1",
            [],
            |row| row.get(0),
        )
        .expect("query last_pr_review_submitted_at");
    assert!(val.is_none(), "last_pr_review_submitted_at should be NULL after migration");
}
```

#### テスト: `fixture_reaches_current_schema`

```rust
#[test]
fn fixture_reaches_current_schema() {
    let (conn, _dir) = load_fixture("v0001-initial");
    conn.init_schema().expect("init_schema");

    let expected_raw = std::fs::read_to_string(
        "tests/migrations/snapshots/current-schema.sql",
    )
    .expect("read current-schema.sql");

    let actual = conn.dump_schema();
    let expected = normalize_schema(&expected_raw);
    let actual_norm = normalize_schema(&actual);

    assert_eq!(
        actual_norm, expected,
        "schema mismatch after migration from v0001-initial"
    );
}
```

---

### `v0001-initial.db` fixture の作成方法

fixture は rusqlite を使ってプログラマティックに生成し、バイナリファイルとして git に commit する。以下の手順でファイルを作成する:

1. `tests/migrations/fixtures/` ディレクトリを作成
2. 以下の Rust コードを実行して fixture を生成（実装時に一時スクリプトとして利用）:

```rust
use rusqlite::Connection;

let path = "tests/migrations/fixtures/v0001-initial.db";
let conn = Connection::open(path).unwrap();
conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;").unwrap();
conn.execute_batch("
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
    );
    CREATE TABLE process_runs (
        id            INTEGER PRIMARY KEY,
        issue_id      INTEGER NOT NULL REFERENCES issues(id),
        type          TEXT NOT NULL,
        idx           INTEGER NOT NULL DEFAULT 0,
        state         TEXT NOT NULL DEFAULT 'running',
        pid           INTEGER,
        pr_number     INTEGER,
        causes        TEXT NOT NULL DEFAULT '[]',
        error_message TEXT,
        started_at    TEXT NOT NULL DEFAULT (datetime('now')),
        finished_at   TEXT
    );
    CREATE INDEX idx_process_runs_issue_id ON process_runs(issue_id);
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
    CREATE INDEX idx_execution_log_issue_id ON execution_log(issue_id);
").unwrap();
conn.execute_batch("
    INSERT INTO issues (github_issue_number, state, feature_name, weight, created_at, updated_at)
    VALUES
        (1, 'idle',           'issue-1', 'medium', datetime('now'), datetime('now')),
        (2, 'design_running', 'issue-2', 'heavy',  datetime('now'), datetime('now'));
").unwrap();
```

この手順は `#[test] #[ignore]` 付きの fixture 生成テストとして `tests/migrations/mod.rs` に記述し、`cargo test -- --ignored generate_fixtures` で一度だけ実行してファイルを生成後、commit する。

---

### `snapshots/current-schema.sql` の生成方法

フレッシュな in-memory DB に `init_schema()` を適用し `dump_schema()` の出力を保存する:

```rust
let db = SqliteConnection::open_in_memory().unwrap();
db.init_schema().unwrap();
let schema = db.dump_schema();
std::fs::write("tests/migrations/snapshots/current-schema.sql", schema).unwrap();
```

同様に `#[test] #[ignore]` 付きの生成テストとして記述し、一度実行後に commit する。

---

### `Cargo.toml` の変更

```toml
[[test]]
name = "migrations"
path = "tests/migrations/mod.rs"
```

既存の `[dev-dependencies]` に `tempfile = "3"` が既に含まれているため、追加の依存関係は不要。

---

### `CHANGELOG.md` の変更

`[Unreleased]` セクションに以下を追加:

```markdown
### Added
- DB マイグレーション回帰テスト (`tests/migrations/`) を追加 ([#327])
  - `SqliteConnection::dump_schema()` テスト用ヘルパーを追加 (`#[cfg(test)]`)
  - `v0001-initial` fixture と `current-schema.sql` スナップショットを追加
  - `migrate_v0001_is_idempotent` / `fixture_reaches_current_schema` テストを追加

### Note
- スキーマを変更する PR では `tests/migrations/fixtures/` に新規 fixture を追加すること
  (`docs/tests/migration-test.md` の section 3 / 5 を参照)
```

## Error Handling

| エラーケース | 箇所 | 対応方針 |
|-------------|------|---------|
| fixture ファイルが存在しない | `load_fixture()` の `fs::copy` | `.expect("copy fixture")` でパニック（テスト環境セットアップエラーとして明示） |
| `sqlite_master` クエリ失敗 | `dump_schema()` | `.expect("query dump_schema")` でパニック（回復不能なテスト環境エラー） |
| `current-schema.sql` 読み取り失敗 | `fixture_reaches_current_schema` | `.expect("read current-schema.sql")` でパニック |
| スキーマ不一致 | `fixture_reaches_current_schema` | `assert_eq!` で差分を含むエラーメッセージを表示して失敗 |

テストコード内では `expect_used` clippy lint が deny になっているが、テストファイル (`tests/`) は `[lints.clippy]` の適用外であるため問題ない。

## Testing Strategy

本 issue 自体がテスト実装であるため、追加のテストは不要。ただし、以下の点を実装時に確認する:

- `devbox run -- cargo test --test migrations` が秒単位で green になること
- `cargo test` （引数なし）でも migrations テストが自動実行されること
- `cargo clippy` / `cargo fmt --check` が pass すること（`tests/` も対象）
