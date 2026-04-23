# 要件ドキュメント

## はじめに

`docs/tests/migration-test.md` に DB マイグレーション回帰テストの完全な仕様が定義されているが、実装が存在しない。本要件は同仕様書に従い、以下を実装する:

- `SqliteConnection::dump_schema()` テストヘルパー
- `tests/migrations/` テストインフラストラクチャ (mod.rs・fixtures・snapshots)
- `v0001-initial.db` フィクスチャとスキーマスナップショット
- `migrate_v0001_is_idempotent` および `fixture_reaches_current_schema` テスト
- Cargo.toml への統合登録
- CHANGELOG への運用ルール追記

## 要件

### 要件 1: dump_schema() テストヘルパー

**目標**: テスト実装者として、マイグレーション後のスキーマをスナップショットファイルと比較できるように、`SqliteConnection` に `dump_schema()` テストヘルパーを追加したい。そうすることで、`fixture_reaches_current_schema` テストを実装できる。

#### 受け入れ条件

1. The migration test infrastructure shall provide a `dump_schema()` method on `SqliteConnection` declared inside a `#[cfg(test)]` block that queries `sqlite_master` and returns all `CREATE TABLE` and `CREATE INDEX` statements as a single string.
2. When `dump_schema()` is called, the migration test infrastructure shall return statements ordered so that tables appear before indexes, and within each type items are sorted in ascending order by name.
3. The migration test infrastructure shall provide a `normalize_schema(sql: &str) -> String` helper that replaces CRLF with LF and strips trailing whitespace from each line.

### 要件 2: テストインフラストラクチャの構築

**目標**: テスト実装者として、マイグレーション回帰テストを体系的に管理できるように、`docs/tests/migration-test.md` のセクション 2 に示されたディレクトリ構成でテストインフラを構築したい。そうすることで、将来の schema バージョン追加も統一された手順で対応できる。

#### 受け入れ条件

1. The migration test infrastructure shall provide the directory structure `tests/migrations/mod.rs`, `tests/migrations/fixtures/`, and `tests/migrations/snapshots/` as defined in `migration-test.md` section 2.
2. The migration test infrastructure shall register the migrations integration test in `Cargo.toml` with a `[[test]]` section specifying `name = "migrations"` and `path = "tests/migrations/mod.rs"`.
3. When `devbox run -- cargo test --test migrations` is executed, the migration test infrastructure shall complete all tests within a few seconds without requiring GitHub or network access.
4. The migration test infrastructure shall provide a `load_fixture(name: &str) -> SqliteConnection` helper in `tests/migrations/mod.rs` that copies the named fixture binary to a `tempfile::NamedTempFile` and returns an opened `SqliteConnection`.

### 要件 3: スキーマスナップショットの管理

**目標**: テスト実装者として、マイグレーション後の期待スキーマを基準値として持てるように、`tests/migrations/snapshots/current-schema.sql` を管理したい。そうすることで、フィクスチャに `init_schema()` を適用した後の結果を基準値と比較できる。

#### 受け入れ条件

1. The migration test infrastructure shall provide `tests/migrations/snapshots/current-schema.sql` containing the normalized DDL output obtained by applying `init_schema()` to a fresh in-memory `SqliteConnection`.
2. When the normalized content of `current-schema.sql` is compared with the normalized output of `dump_schema()` on a fully-migrated fixture, the migration test infrastructure shall treat them as equal when the schemas match.

### 要件 4: v0001-initial フィクスチャの作成

**目標**: テスト実装者として、古いスキーマ状態 (ALTER TABLE 追加前) を持つフィクスチャを利用できるように、`v0001-initial.db` を用意したい。そうすることで、古い DB から現行スキーマへのマイグレーションパスを検証できる。

#### 受け入れ条件

1. The migration test infrastructure shall provide `tests/migrations/fixtures/v0001-initial.db` as a valid SQLite binary file representing the pre-migration `issues` table schema — a schema that lacks the newer columns added via `ALTER TABLE` (e.g., `weight`, `ci_fix_count`, `ci_fix_limit_notified`, `close_finished`, `consecutive_failures_epoch`).
2. When the v0001-initial fixture is loaded, it shall contain at least one `issues` row with `state = 'idle'` (github_issue_number = 1) and at least one row with `state = 'design_running'` (github_issue_number = 2).
3. The migration test infrastructure shall provide `tests/migrations/fixtures/v0001-initial.README.md` documenting the fixture's origin, contained tables, row counts, schema version, and verification intent.

### 要件 5: マイグレーションテストの実装

**目標**: テスト実装者として、`init_schema()` の冪等性と過去 DB から現行スキーマへの移行を自動検証できるように、`migration-test.md` セクション 4.1・4.3 のテンプレートに従った 2 本の回帰テストを実装したい。

#### 受け入れ条件

1. When the `migrate_v0001_is_idempotent` test is executed, the migration test system shall apply `init_schema()` twice to the v0001-initial fixture, verify that all pre-existing rows are preserved with correct field values, and verify that newly-added columns have the expected default values (e.g., `close_finished = 0`).
2. When the `fixture_reaches_current_schema` test is executed, the migration test system shall apply `init_schema()` to the v0001-initial fixture and assert that the normalized `dump_schema()` output equals the normalized `current-schema.sql` content, failing with a descriptive diff message on mismatch.
3. If either migration test fails, the test name shall clearly identify which fixture triggered the failure.
4. While executing migration tests, the migration test system shall use `tempfile::NamedTempFile` to work on copies of fixture files, ensuring the original fixture binary is never modified.

### 要件 6: CI 統合

**目標**: CI 担当者として、マイグレーションテストを既存のビルド検証フローに追加のコマンドなしで統合できるように、`cargo test` の標準実行パスにマイグレーションテストを含めたい。

#### 受け入れ条件

1. When `devbox run test` (equivalent to `cargo test`) is executed without explicit target filtering, the migration test infrastructure shall include the `migrations` integration test target in the test run.
2. The migration test infrastructure shall pass `devbox run -- cargo fmt --check` and `devbox run clippy` without warnings or errors in any newly added source files.

### 要件 7: 運用ルールのドキュメント化

**目標**: 開発者として、schema 変更時に fixture を追加する運用ルールをチームが遵守できるように、`CHANGELOG.md` に明記したい。そうすることで、マイグレーションテストカバレッジが schema 変更時に必ず更新される。

#### 受け入れ条件

1. The migration test infrastructure shall add an entry in `CHANGELOG.md` under `[Unreleased]` > `Added` section documenting that a migration fixture must be added to `tests/migrations/fixtures/` whenever the schema (`init_schema`) is changed, referencing `migration-test.md` for the procedure.
