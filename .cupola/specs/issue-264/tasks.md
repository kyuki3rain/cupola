# 実装計画

- [x] 1. 共通ヘルパーモジュールを新設する
- [x] 1.1 `sqlite_helpers.rs` を作成し、共通関数を定義する
  - `src/adapter/outbound/sqlite_helpers.rs` を新規作成する
  - `sqlite_issue_repository.rs` の `pub fn str_to_state` と同一の実装を `pub(super) fn str_to_state` として定義する
  - 両リポジトリに重複している `fn parse_sqlite_datetime` の実装を `pub(super) fn parse_sqlite_datetime` として定義する
  - 必要な `use` 宣言（`rusqlite`、`chrono::DateTime`、`chrono::Utc`、`crate::domain::state::State`）を追加する
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 1.2 `mod.rs` にモジュール宣言を追加する
  - `src/adapter/outbound/mod.rs` に `mod sqlite_helpers;` を追加する（`pub mod` にしない）
  - _Requirements: 1.4_

- [x] 2. リポジトリを共通モジュール参照へ移行する
- [x] 2.1 `sqlite_issue_repository.rs` を更新する
  - `use super::sqlite_helpers::{str_to_state, parse_sqlite_datetime};` の import を追加する
  - ファイル末尾に定義されている `pub fn str_to_state` と `fn parse_sqlite_datetime` の定義を削除する
  - `row_to_issue` 関数内の `parse_sqlite_datetime` 呼び出しがそのまま動作することを確認する（import 変更のみで動作不変）
  - テストモジュールは現行どおり `use super::*;` を維持し、追加の import 書き換えは不要であることを確認する（`str_to_state(...)` 呼び出しは親モジュールが `use super::sqlite_helpers::str_to_state;` を通じて再エクスポートする形で解決される前提）
  - _Requirements: 2.1, 2.4, 3.1, 3.3_

- [x] 2.2 (P) `sqlite_execution_log_repository.rs` を更新する
  - `use super::sqlite_issue_repository::str_to_state;` を `use super::sqlite_helpers::{str_to_state, parse_sqlite_datetime};` に置き換える
  - ファイル末尾に定義されている `fn parse_sqlite_datetime` の定義を削除する
  - `find_by_issue` クロージャ内の `parse_sqlite_datetime` 呼び出しがそのまま動作することを確認する
  - _Requirements: 2.2, 2.3, 3.1, 3.2_

- [x] 3. ビルドとテストを確認する
- [x] 3.1 コンパイルとテストを実行して回帰がないことを確認する
  - `devbox run build` でコンパイルエラーがないことを確認する
  - `devbox run test` で全テストが通過することを確認する
  - `devbox run clippy` で警告がゼロであることを確認する
  - `devbox run fmt-check` でフォーマット差分がないことを確認する
  - _Requirements: 1.5, 3.1, 3.2, 3.4_
