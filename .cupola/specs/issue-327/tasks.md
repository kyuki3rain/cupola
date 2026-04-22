# 実装タスクリスト

## タスク一覧

- [ ] 1. dump_schema() テストヘルパーの実装
- [ ] 1.1 SqliteConnection に dump_schema() を追加する
  - `src/adapter/outbound/sqlite_connection.rs` の既存 `#[cfg(test)] mod tests` の直前に `#[cfg(test)]` impl ブロックを追加する
  - `dump_schema()` メソッドは `conn_lock()` で Mutex ガードを取得し、`sqlite_master` から `type IN ('table', 'index') AND sql IS NOT NULL` の DDL 文を `ORDER BY CASE type WHEN 'table' THEN 0 ELSE 1 END, name ASC` で取得し、`";\n"` で連結して末尾に `";"` を付与して返す
  - `#[cfg_attr(test, allow(clippy::expect_used))]` が既に有効なので `.expect()` を使用してよい
  - _Requirements: 1.1, 1.2_

- [ ] 1.2 (P) normalize_schema ヘルパー関数を実装する
  - `tests/migrations/mod.rs` に `fn normalize_schema(sql: &str) -> String` を自由関数として実装する
  - `replace("\r\n", "\n")` → `lines()` → `trim_end()` → 空行フィルタ → `join("\n")` の変換を行う
  - _Requirements: 1.3_

- [ ] 2. テストインフラストラクチャの構築
- [ ] 2.1 ディレクトリ構造と Cargo.toml を整備する
  - `tests/migrations/fixtures/` ディレクトリを作成する
  - `tests/migrations/snapshots/` ディレクトリを作成する
  - `tests/migrations/mod.rs` ファイルを作成する (後続タスクで内容を追加)
  - `Cargo.toml` に `[[test]]` セクションを追加し `name = "migrations"` と `path = "tests/migrations/mod.rs"` を指定する
  - _Requirements: 2.1, 2.2, 6.1, 6.2_

- [ ] 2.2 load_fixture ヘルパーを実装する
  - `tests/migrations/mod.rs` に `fn load_fixture(name: &str) -> SqliteConnection` を実装する
  - `format!("tests/migrations/fixtures/{name}.db")` でソースパスを構築する
  - `tempfile::NamedTempFile::new()` で一時ファイルを作成し `std::fs::copy` でコピーする
  - コピー先パスで `SqliteConnection::open()` を呼び出して返す
  - _Requirements: 2.4, 5.4_

- [ ] 3. v0001-initial フィクスチャの作成
- [ ] 3.1 フィクスチャ生成ヘルパーを実装してバイナリを生成する
  - `tests/migrations/mod.rs` に `#[test] #[ignore] fn create_v0001_fixture()` を実装する
  - 旧 `issues` テーブルスキーマ (id, github_issue_number, state, worktree_path, created_at, updated_at のみ) で DB を構築する
  - `github_issue_number = 1, state = 'idle'` と `github_issue_number = 2, state = 'design_running'` の 2 行を INSERT する
  - `std::fs::copy` でバイナリを `tests/migrations/fixtures/v0001-initial.db` に書き出す
  - `devbox run -- cargo test -- --ignored create_v0001_fixture` を実行してバイナリを生成し、git add する
  - _Requirements: 4.1, 4.2_

- [ ] 3.2 (P) v0001-initial.README.md を作成する
  - `tests/migrations/fixtures/v0001-initial.README.md` を作成する
  - 含まれるテーブル、行数、スキーマバージョン (pre-ALTER TABLE migration)、作成手順、検証ポイントを記載する
  - _Requirements: 4.3_

- [ ] 4. スキーマスナップショットの初期化
- [ ] 4.1 current-schema.sql を生成する
  - `tests/migrations/mod.rs` に `#[test] #[ignore] fn update_current_schema_snapshot()` を実装する
  - フレッシュな `SqliteConnection::open_in_memory()` に `init_schema()` を適用し、`dump_schema()` を呼んで `normalize_schema()` を通した結果を `tests/migrations/snapshots/current-schema.sql` に書き出す
  - `devbox run -- cargo test -- --ignored update_current_schema_snapshot` を実行してファイルを生成し、git add する
  - _Requirements: 3.1, 3.2_

- [ ] 5. マイグレーションテストの実装
- [ ] 5.1 migrate_v0001_is_idempotent テストを実装する
  - `tests/migrations/mod.rs` に `#[test] fn migrate_v0001_is_idempotent()` を実装する
  - `load_fixture("v0001-initial")` でフィクスチャを読み込む
  - `init_schema()` を 2 回呼んで冪等性を確認する
  - `conn_lock()` 経由で `SELECT github_issue_number, state FROM issues ORDER BY github_issue_number` を実行し、2 行存在すること・`rows[0].state == "idle"` を検証する
  - `SELECT close_finished FROM issues WHERE github_issue_number = 1` でデフォルト値 `0` を検証する
  - _Requirements: 5.1, 5.3, 5.4_

- [ ] 5.2 (P) fixture_reaches_current_schema テストを実装する
  - `tests/migrations/mod.rs` に `#[test] fn fixture_reaches_current_schema()` を実装する
  - `load_fixture("v0001-initial")` でフィクスチャを読み込み `init_schema()` を適用する
  - `dump_schema()` の結果と `std::fs::read_to_string("tests/migrations/snapshots/current-schema.sql")` の内容をそれぞれ `normalize_schema()` にかけて `assert_eq!` する
  - _Requirements: 5.2, 5.3, 5.4_

- [ ] 6. CI 検証と CHANGELOG 更新
- [ ] 6.1 テスト実行と品質チェックを完了する
  - `devbox run -- cargo test --test migrations` がすべて green になることを確認する
  - `devbox run fmt` と `devbox run clippy` が新規ファイルに対してエラーを出さないことを確認する
  - _Requirements: 2.3, 6.1, 6.2_

- [ ] 6.2 (P) CHANGELOG.md を更新する
  - `CHANGELOG.md` の `[Unreleased]` > `Added` セクションに「`init_schema()` を変更するたびに `tests/migrations/fixtures/` に新しいフィクスチャを追加すること (`docs/tests/migration-test.md` 参照)」を追記する
  - _Requirements: 7.1_
