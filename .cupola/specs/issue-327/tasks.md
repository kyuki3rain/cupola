# 実装タスク

- [x] 1. `dump_schema()` テスト用ヘルパーを追加する
- [x] 1.1 `SqliteConnection` に `#[cfg(test)]` 付きの `dump_schema()` メソッドを追加する
  - `src/adapter/outbound/sqlite_connection.rs` の末尾近くに `#[cfg(test)]` impl ブロックを追加する
  - `sqlite_master` から `type IN ('table', 'index') AND sql IS NOT NULL ORDER BY name` でクエリし、セミコロン区切りで連結した文字列を返す
  - 既存の `#[cfg(test)] mod tests` ブロックとは別の impl ブロックとして配置する（メソッドはテストコードから直接呼ばれるため `mod tests` の外に置く）
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 2. マイグレーションテスト基盤を整備する
- [x] 2.1 `tests/migrations/` ディレクトリ構造を作成する
  - `tests/migrations/fixtures/` ディレクトリを作成する
  - `tests/migrations/snapshots/` ディレクトリを作成する
  - _Requirements: 2.1_

- [x] 2.2 `v0001-initial.db` fixture を生成して commit する
  - `tests/migrations/mod.rs` に `#[test] #[ignore]` 付きの `generate_v0001_fixture()` 関数を追加する
  - 関数内で rusqlite::Connection を使って `last_pr_review_submitted_at` / `body_hash` を含まない旧スキーマの DB ファイルを `tests/migrations/fixtures/v0001-initial.db` に生成する
  - issues テーブルに `(github_issue_number=1, state='idle', feature_name='issue-1', weight='medium')` と `(github_issue_number=2, state='design_running', feature_name='issue-2', weight='heavy')` の 2 行を挿入する
  - `cargo test -- --ignored generate_v0001_fixture` を実行してファイルを生成し、バイナリファイルとして git add する
  - _Requirements: 2.2, 2.3_

- [x] 2.3 `v0001-initial.README.md` を作成する
  - `tests/migrations/fixtures/v0001-initial.README.md` に fixture の作成経緯・含まれるテーブル/行数・検証ポイントを記述する
  - _Requirements: 2.4_

- [x] 2.4 `snapshots/current-schema.sql` を生成して commit する
  - `tests/migrations/mod.rs` に `#[test] #[ignore]` 付きの `generate_current_schema_snapshot()` 関数を追加する
  - 関数内でフレッシュな in-memory DB に `init_schema()` を適用し `dump_schema()` 出力を `tests/migrations/snapshots/current-schema.sql` に書き出す
  - `cargo test -- --ignored generate_current_schema_snapshot` を実行してファイルを生成し、git add する
  - _Requirements: 2.5_

- [x] 3. `tests/migrations/mod.rs` にテストを実装する
- [x] 3.1 `load_fixture()` ヘルパーを実装する
  - `(SqliteConnection, tempfile::TempDir)` を返す関数として実装し、`TempDir` によって一時ファイルの生存期間を管理する
  - _Requirements: 3.1, 4.1_

- [x] 3.2 `normalize_schema()` ヘルパーを実装する
  - CRLF→LF・末尾空白除去・空行除去を行う関数として実装する
  - _Requirements: 4.3_

- [x] 3.3 `migrate_v0001_is_idempotent` テストを実装する
  - `load_fixture("v0001-initial")` を使って fixture を読み込む
  - `init_schema()` を 2 回呼んでエラー・パニックが発生しないことを確認する
  - `github_issue_number=1` の `state` が `'idle'` であることを検証する
  - `github_issue_number=2` の `state` が `'design_running'` であることを検証する
  - `github_issue_number=1` の `last_pr_review_submitted_at` が NULL であることを検証する
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [x] 3.4 `fixture_reaches_current_schema` テストを実装する
  - `load_fixture("v0001-initial")` を使って fixture を読み込む
  - `init_schema()` を 1 回呼んで `dump_schema()` を取得する
  - `current-schema.sql` を読み込み、両方を `normalize_schema()` で正規化して比較する
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 4. `Cargo.toml` に `[[test]]` セクションを追加する
  - `[[test]] name = "migrations" path = "tests/migrations/mod.rs"` を追加する
  - `devbox run -- cargo test --test migrations` が green になることを確認する
  - _Requirements: 5.1, 5.2, 5.3_

- [x] 5. `CHANGELOG.md` を更新する
  - `[Unreleased]` セクションにマイグレーション回帰テスト追加の記録を追記する
  - スキーマ変更時は fixture を追加する旨の注記を追加する
  - _Requirements: 6.1, 6.2_
