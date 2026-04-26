# fix-silent-fallback-to-result サマリー

## Feature
`sqlite_issue_repository.rs` / `sqlite_connection.rs` のサイレントフォールバックを `rusqlite::Error` 伝播に変換し DB データ破損を早期検知する。

## 要件サマリ
1. `str_to_state` の未知文字列 → `Idle` フォールバックを廃止、`Err(InvalidColumnType)` を返す。
2. `fixing_causes_json` カラム取得を `row.get(11)?` に。
3. `fixing_causes` JSON パース失敗を `FromSqlConversionFailure` で伝播。
4. `parse_sqlite_datetime` の epoch フォールバックを廃止し `InvalidColumnType` を返す。
5. `fixing_causes` マイグレーションを `run_add_column_migration` に統一。
6. `reset_for_restart` の UPDATE 文に `fixing_causes = '[]'` を追加。

## アーキテクチャ決定
- 変更は adapter/outbound 層 2 ファイルに閉じ、`IssueRepository` trait や呼び出し元のシグネチャは維持。`row_to_issue` は既に `rusqlite::Result<Issue>` を返すため内部実装の置き換えで済む。
- エラー型はカスタム型を導入せず `rusqlite::Error` のみを使用。`serde_json::Error` は `FromSqlConversionFailure(col_idx, Type::Text, Box::new(e))` に変換。
- `query_map(...).collect::<Result<Vec<_>, _>>()` により破損行 1 件でもクエリ全体を `Err` にする。これが「明示的検知」という修正目的に合致。

## コンポーネント
- `adapter/outbound/sqlite_issue_repository.rs`: `str_to_state`, `row_to_issue`, `parse_sqlite_datetime`, `reset_for_restart`
- `adapter/outbound/sqlite_connection.rs`: `init_schema` マイグレーション統一

## 主要インターフェース
- `fn str_to_state(col_idx: usize, s: &str) -> rusqlite::Result<State>`
- `fn parse_sqlite_datetime(col_idx: usize, s: &str) -> rusqlite::Result<DateTime<Utc>>`
- `fn row_to_issue(row: &Row) -> rusqlite::Result<Issue>`（戻り型既存維持）

## 学び/トレードオフ
- サイレントフォールバックは破損 Issue が Idle として再処理され worktree/PR 再作成、epoch による stall timeout 誤判定といった不具合を誘発するため明示エラー化のメリット大。
- 既存 `tracing::warn!` は削除し呼び出し側の `anyhow::Context` に集約。
- スキーマ変更なし・冪等性維持のためロールバックは単純に元に戻すだけ。
