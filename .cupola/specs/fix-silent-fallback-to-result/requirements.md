# Requirements Document

## Introduction

`sqlite_issue_repository.rs` の `row_to_issue` 関連関数では、DBデータの異常（不明な state 文字列、カラム取得失敗、JSON パース失敗、日時パース失敗）をサイレントに飲み込み、デフォルト値にフォールバックしている。これにより、破損したデータを持つ Issue が「Idle 状態」として再処理されたり、stall timeout が誤判定されるなど、予期しない動作を引き起こすリスクがある。

本仕様では、これらのサイレントフォールバックをすべて `Result` へのエラー伝播に変更し、DBデータ異常時は適切なエラーログを出力したうえで `row_to_issue` / `find_active` などのクエリ処理全体を `Err` として失敗させ、呼び出し元へ伝播する挙動に統一する。また、関連する `sqlite_connection.rs` のマイグレーションエラー無視と `reset_for_restart` の `fixing_causes` リセット漏れも合わせて修正する。

## Requirements

### 1. str_to_state のエラー伝播

**Objective:** Cupola エージェントの開発者として、不明な state 文字列が DB に格納されていた場合にエラーとして検出できるようにしたい。これにより、データ破損による Issue の意図しない再処理（worktree 再作成・PR 再作成等）を防止できる。

#### Acceptance Criteria

1. When `str_to_state` が未知の state 文字列を受け取ったとき, the SqliteIssueRepository shall `rusqlite::Error`（`InvalidColumnType` 相当）を返す（`Idle` へのフォールバックは行わない）
2. When `row_to_issue` が `str_to_state` からエラーを受け取ったとき, the SqliteIssueRepository shall `?` 演算子でそのエラーを呼び出し元へ伝播する
3. If `str_to_state` が有効な state 文字列（`Idle`、`Designing` 等）を受け取ったとき, the SqliteIssueRepository shall 対応する `State` 列挙子を `Ok(...)` で返す
4. The SqliteIssueRepository shall `str_to_state` の戻り型を `Result<State, rusqlite::Error>` とする

### 2. fixing_causes_json カラム取得のエラー伝播

**Objective:** Cupola エージェントの開発者として、`fixing_causes` カラムの取得失敗をサイレントに無視せず検出できるようにしたい。これにより、データ不整合を早期に発見できる。

#### Acceptance Criteria

1. When `row_to_issue` 内で `fixing_causes` カラムの取得に失敗したとき, the SqliteIssueRepository shall `?` 演算子でエラーを伝播する（`"[]"` へのフォールバックは行わない）
2. The SqliteIssueRepository shall `fixing_causes` カラム取得を他のカラム（`title`、`state` 等）と同一のパターンで実装する

### 3. fixing_causes JSON パースのエラー伝播

**Objective:** Cupola エージェントの開発者として、`fixing_causes` カラムの JSON パース失敗を検出できるようにしたい。これにより、fixing 原因が消失した状態で処理が継続されることを防ぐ。

#### Acceptance Criteria

1. When `fixing_causes` カラムの値が有効な JSON 配列でないとき, the SqliteIssueRepository shall `rusqlite::Error`（`InvalidColumnType` 相当）を返す（`vec![]` へのフォールバックは行わない）
2. When JSON パースが成功したとき, the SqliteIssueRepository shall パース結果の `Vec<FixingCause>` を `Ok(...)` で返す
3. The SqliteIssueRepository shall `serde_json::from_str` の失敗を `rusqlite::Error` に変換して伝播する

### 4. parse_sqlite_datetime のエラー伝播

**Objective:** Cupola エージェントの開発者として、日時のパース失敗を検出できるようにしたい。これにより、epoch（1970-01-01）へのフォールバックによる stall timeout の誤判定を防ぐ。

#### Acceptance Criteria

1. When 日時文字列のパースに失敗したとき, the SqliteIssueRepository shall `rusqlite::Error`（`InvalidColumnType` 相当）を返す（epoch へのフォールバックは行わない）
2. When 日時文字列のパースに成功したとき, the SqliteIssueRepository shall `Ok(DateTime<Utc>)` を返す
3. The SqliteIssueRepository shall `parse_sqlite_datetime` の戻り型を `Result<DateTime<Utc>, rusqlite::Error>` とする
4. When `row_to_issue` が `parse_sqlite_datetime` からエラーを受け取ったとき, the SqliteIssueRepository shall `?` 演算子でエラーを伝播する

### 5. fixing_causes マイグレーションのエラー処理

**Objective:** Cupola エージェントの開発者として、`fixing_causes` カラムのマイグレーション失敗を他のカラムと同様に処理できるようにしたい。これにより、マイグレーションエラーが無視されてスキーマ不整合が生じることを防ぐ。

#### Acceptance Criteria

1. When `fixing_causes` カラムの追加マイグレーション実行中にエラーが発生したとき, the SqliteConnection shall そのエラーを呼び出し元へ伝播する（`let _ = ...` で無視しない）
2. The SqliteConnection shall `fixing_causes` カラムのマイグレーションを `run_add_column_migration` 関数を用いて実装する
3. The SqliteConnection shall `fixing_causes` のマイグレーション実装を既存の他カラム（`design_pr_number` 等）と同一パターンで統一する

### 6. reset_for_restart における fixing_causes のリセット

**Objective:** Cupola エージェントの開発者として、Issue を再起動状態にリセットする際に `fixing_causes` も確実にリセットされるようにしたい。これにより、データの衛生状態を保ち、将来的なバグの温床を除去できる。

#### Acceptance Criteria

1. When `reset_for_restart` が呼び出されたとき, the SqliteIssueRepository shall `fixing_causes` カラムを `'[]'` にリセットする SQL を実行する
2. The SqliteIssueRepository shall `fixing_causes` のリセットを他のカラム（`worktree_path`、`design_pr_number` 等）と同一の UPDATE 文で行う
3. When リセット後に Issue を取得したとき, the SqliteIssueRepository shall `fixing_causes` が空リスト（`[]`）であることを返す
