# Implementation Plan

- [x] 1. sqlite_issue_repository のサイレントフォールバック修正

- [x] 1.1 str_to_state の戻り型を Result に変更する
  - `str_to_state` が未知の state 文字列を受け取ったとき `rusqlite::Error::InvalidColumnType` を返すよう変更する
  - 既知の全 state 文字列（idle, initialized, design_running, design_review_waiting, design_fixing, implementation_running, implementation_review_waiting, implementation_fixing, completed, cancelled）はそれぞれ `Ok(State::Xxx)` を返す
  - 関数シグネチャを `fn str_to_state(col_idx: usize, s: &str) -> rusqlite::Result<State>` に変更する
  - `_` のワイルドカードアームを削除し、unknown 文字列に対して `Err(rusqlite::Error::InvalidColumnType(col_idx, s.to_owned(), rusqlite::types::Type::Text))` を返す
  - `row_to_issue` 内の呼び出しを `str_to_state(2, &state_str)?` に更新する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 1.2 fixing_causes_json カラム取得のエラー伝播に変更する
  - `row.get(11).unwrap_or_else(|_| "[]".to_string())` を `row.get(11)?` に変更する
  - 他のカラム取得と同一パターン（`row.get(N)?`）に統一する
  - _Requirements: 2.1, 2.2_

- [x] 1.3 fixing_causes JSON パース失敗を Result エラーとして伝播する
  - `serde_json::from_str(...).unwrap_or_else(|e| { tracing::warn!(...); vec![] })` を削除する
  - `serde_json::from_str::<Vec<FixingProblemKind>>(&fixing_causes_json).map_err(|e| rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(e)))?` に変更する
  - `tracing::warn!` によるサイレント警告ログを削除する（呼び出しスタック上でエラーログが記録されるため）
  - _Requirements: 3.1, 3.2, 3.3_

- [x] 1.4 parse_sqlite_datetime の戻り型を Result に変更する
  - `parse_sqlite_datetime` の戻り型を `rusqlite::Result<DateTime<Utc>>` に変更する
  - `unwrap_or_default()` を削除し、パース失敗時は `Err(rusqlite::Error::InvalidColumnType(col_idx, s.to_owned(), rusqlite::types::Type::Text))` を返す
  - 関数シグネチャを `fn parse_sqlite_datetime(col_idx: usize, s: &str) -> rusqlite::Result<DateTime<Utc>>` に変更する
  - `row_to_issue` 内の呼び出しを `parse_sqlite_datetime(12, &created_str)?` / `parse_sqlite_datetime(13, &updated_str)?` に更新する
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 1.5 reset_for_restart の UPDATE SQL に fixing_causes リセットを追加する
  - `reset_for_restart` の UPDATE 文に `fixing_causes = '[]',` を追加する
  - 他のカラム（model, error_message 等）と同じ UPDATE 文の中でリセットを行う
  - _Requirements: 6.1, 6.2_

- [x] 2. (P) sqlite_connection のマイグレーション処理を統一する
  - `let _ = conn.execute_batch("ALTER TABLE issues ADD COLUMN fixing_causes TEXT NOT NULL DEFAULT '[]';")` を削除する
  - `Self::run_add_column_migration(&conn, "fixing_causes TEXT NOT NULL DEFAULT '[]'")?` に置き換える
  - `feature_name`・`model` カラムのマイグレーションと同一パターンになっていることを確認する
  - _Requirements: 5.1, 5.2, 5.3_

- [x] 3. テストで修正内容を検証する

- [x] 3.1 str_to_state と parse_sqlite_datetime のユニットテストを追加する
  - `str_to_state` に既知の全 state 文字列を渡したとき対応する `State` が返ることを確認する
  - `str_to_state` に未知の文字列を渡したとき `Err` が返ることを確認する
  - `parse_sqlite_datetime` に有効な日時文字列を渡したとき `Ok(DateTime)` が返ることを確認する
  - `parse_sqlite_datetime` に無効な文字列を渡したとき `Err` が返ることを確認する
  - _Requirements: 1.1, 1.3, 4.1, 4.2_

- [x] 3.2 破損データを含む Row が find_active / find_needing_process でエラーを返すことを検証する
  - インメモリ DB に状態が不明文字列の Issue を直接 INSERT する
  - `find_active` または `find_needing_process` を呼んだとき `Err` が返ることを確認する
  - 破損 JSON（`fixing_causes` が `"not_json"`）の場合も同様に `Err` を返すことを確認する
  - _Requirements: 1.2, 3.1_

- [x] 3.3 reset_for_restart 後の fixing_causes が空配列になることを確認する
  - Issue を保存した後、`fixing_causes` に値を設定して更新する
  - `reset_for_restart` を呼び出した後、Issue を再取得して `fixing_causes` が空配列であることを確認する
  - _Requirements: 6.1, 6.3_

- [x] 3.4 init_schema の冪等性テストで fixing_causes カラムの重複追加が無視されることを確認する
  - `init_schema` を2回呼び出してもエラーにならないことを確認する（既存テストが通過すれば可）
  - _Requirements: 5.1_
