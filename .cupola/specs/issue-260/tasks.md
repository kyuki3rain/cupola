# Implementation Plan

- [ ] 1. `SqliteConnection` に `conn_lock()` ヘルパーメソッドを追加する
- [ ] 1.1 `conn_lock()` メソッドの実装とコメント追加
  - `SqliteConnection` に `pub fn conn_lock(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection>` を追加する
  - 実装には `self.conn.lock().unwrap_or_else(|e| panic!("database Mutex poisoned: a previous thread panicked while holding the lock: {e}"))` を使用する
  - ロック毒化が致命的な理由を説明する doc コメントを追加する（`# Panics` セクション含む）
  - `init_schema()` 内の既存の `.lock().map_err(...)` 呼び出しを `conn_lock()` に置き換える
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 1.2 `conn_lock()` の単体テスト追加
  - `sqlite_connection.rs` のテストモジュールに `conn_lock_panics_on_poisoned_mutex` テストを追加する（`#[should_panic(expected = "poisoned")]`）
  - Mutex毒化の再現: `std::thread::spawn(|| { let _g = conn.lock().unwrap(); panic!(); }).join()` のパターンを使用する
  - `conn_lock_succeeds_on_healthy_mutex` テストで正常動作を検証する
  - _Requirements: 2.1, 2.3_

- [ ] 2. 各リポジトリの `.lock().map_err(...)` を `conn_lock()` に置き換える
- [ ] 2.1 (P) `sqlite_issue_repository.rs` の置き換え
  - すべての `db.conn().lock().map_err(|e| anyhow::anyhow!(...))?\n` を `db.conn_lock()` に置き換える（末尾の `?` も除去する）
  - 影響する全メソッド: `find_by_id`, `find_by_issue_number`, `find_active`, `find_all`, `save`, `update_state`, `update`, `update_state_and_metadata`, `find_by_state`
  - _Requirements: 1.2, 2.2_

- [ ] 2.2 (P) `sqlite_process_run_repository.rs` の置き換え
  - すべての `db.conn().lock().map_err(|e| anyhow::anyhow!(...))?\n` を `db.conn_lock()` に置き換える（末尾の `?` も除去する）
  - 影響する全メソッド: `save`, `update_pid`, `mark_succeeded`, `mark_failed`, `mark_stale`, `mark_stale_for_issue`, `find_latest`, `find_latest_with_pr_number`, `find_by_issue`, `count_consecutive_failures`, `find_latest_with_consecutive_count`, `find_all_running`, `update_state`
  - _Requirements: 1.2, 2.2_

- [ ] 2.3 (P) `sqlite_execution_log_repository.rs` の置き換え
  - すべての `db.conn().lock().map_err(|e| anyhow::anyhow!(...))?\n` を `db.conn_lock()` に置き換える（末尾の `?` も除去する）
  - 影響する全メソッド: `record_start`, `record_finish`, `find_by_issue`
  - _Requirements: 1.2, 2.2_

- [ ] 3. ビルド・テスト・Clippy 検証
- [ ] 3.1 全テストの通過確認
  - `devbox run test` で全テストが通過することを確認する
  - 既存のリポジトリテスト（`sqlite_issue_repository`、`sqlite_process_run_repository`、`sqlite_execution_log_repository`）が引き続きパスすることを確認する（リグレッションなし）
  - _Requirements: 2.2_

- [ ] 3.2 Clippy・フォーマット検証
  - `devbox run clippy` で警告・エラーがないことを確認する（特に `expect_used = "deny"` に抵触していないことを検証）
  - `devbox run fmt-check` でフォーマットが正しいことを確認する
  - _Requirements: 1.1, 1.2_
