# 要件定義書

## はじめに

`sqlite_issue_repository.rs` と `sqlite_execution_log_repository.rs` の両ファイルに、SQLite 操作に関する共通ヘルパー関数が重複して定義されている。`parse_sqlite_datetime` は両ファイルにまったく同じ実装が存在し、`str_to_state` は `sqlite_issue_repository.rs` に `pub` として定義され `sqlite_execution_log_repository.rs` から直接参照される（兄弟モジュール間の不適切な結合）。本リファクタリングでは、これらの共通関数を新設の `sqlite_helpers.rs` モジュールに集約し、重複と不適切な依存を解消する。

## 要件

### 要件 1: 共通ヘルパーモジュールの新設

**目的:** アダプター開発者として、SQLite リポジトリ間で共通するヘルパー関数を一か所で管理できるようにしたい。それにより、将来の変更やバグ修正が一箇所で完結し、各リポジトリの責務が明確になる。

#### 受け入れ基準

1. The system shall provide a new module `src/adapter/outbound/sqlite_helpers.rs` that contains shared SQLite helper functions used by multiple repositories.
2. The system shall define `pub(super) fn str_to_state(col_idx: usize, s: &str) -> rusqlite::Result<State>` in the helpers module, implementing the same logic currently in `sqlite_issue_repository.rs`.
3. The system shall define `pub(super) fn parse_sqlite_datetime(col_idx: usize, s: &str) -> rusqlite::Result<DateTime<Utc>>` in the helpers module, implementing the same parsing logic currently duplicated across both repository files.
4. The system shall declare the helpers module in `src/adapter/outbound/mod.rs` as a non-public (`mod sqlite_helpers`) module.
5. When the helpers module is compiled, the system shall pass `cargo clippy -- -D warnings` and `cargo fmt --check` without errors.

### 要件 2: リポジトリの共通モジュール参照への移行

**目的:** アダプター開発者として、各リポジトリが兄弟モジュールへ直接依存することなく共有ヘルパーを利用できるようにしたい。それにより、モジュール間の結合が疎になり、Clean Architecture のレイヤー規約に沿った設計となる。

#### 受け入れ基準

1. When `sqlite_issue_repository.rs` is compiled, the system shall import `str_to_state` and `parse_sqlite_datetime` from `super::sqlite_helpers` and remove the local definitions of these functions.
2. When `sqlite_execution_log_repository.rs` is compiled, the system shall import `str_to_state` and `parse_sqlite_datetime` from `super::sqlite_helpers` instead of importing `str_to_state` from `super::sqlite_issue_repository`.
3. The system shall ensure that no repository module directly imports helper functions from a sibling repository module after this change.
4. If either repository is modified to remove a local definition that was previously `pub`, the system shall verify that no other module outside the `adapter/outbound` directory referenced that definition.

### 要件 3: 既存機能・テストの無変化保証

**目的:** アダプター開発者として、リファクタリングによって既存のふるまいが変わらないことを確認したい。それにより、安全なコード整理が保証され、回帰リスクをゼロに保てる。

#### 受け入れ基準

1. When all existing tests are executed after the refactoring, the system shall pass all tests that were passing before the change without modification to any test code.
2. The system shall produce identical runtime behavior for `str_to_state` and `parse_sqlite_datetime` calls in both repositories, as verified by existing unit and integration tests.
3. If `str_to_state` was previously exposed as `pub` and used in test modules via `use super::str_to_state`, the system shall keep it accessible to those tests (e.g., via `pub(super)` or by adjusting the import path in the test).
4. When `devbox run test` is executed after the refactoring, the system shall complete with zero failures.
