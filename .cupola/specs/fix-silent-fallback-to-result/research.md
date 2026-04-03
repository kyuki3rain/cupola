# Research & Design Decisions

---
**Purpose**:本仕様の調査記録と設計判断の根拠を記録する。

---

## Summary
- **Feature**: `fix-silent-fallback-to-result`
- **Discovery Scope**: Extension（既存システムへのバグ修正）
- **Key Findings**:
  - 修正対象は `sqlite_issue_repository.rs` と `sqlite_connection.rs` の2ファイルのみで、呼び出し元の変更は不要
  - `row_to_issue` は既に `rusqlite::Result<Issue>` を返しているため、`?` 演算子による伝播パターンは既存コードと一貫している
  - `run_add_column_migration` は既に存在し、`feature_name`・`model` カラムで実績があるため、`fixing_causes` にも同一パターンを適用できる

## Research Log

### row_to_issue の現状分析

- **Context**: サイレントフォールバック箇所の特定
- **Sources Consulted**: `src/adapter/outbound/sqlite_issue_repository.rs` (実コード読解)
- **Findings**:
  - L289: `row.get(11).unwrap_or_else(|_| "[]".to_string())` — カラム取得失敗時に `"[]"` を返す
  - L293-297: `serde_json::from_str(...).unwrap_or_else(|e| { tracing::warn!(...); vec![] })` — JSONパース失敗時に空vecを返す
  - L271-285: `str_to_state` — 戻り型が `State`、unknown文字列が `State::Idle` にマッピングされる
  - L317-321: `parse_sqlite_datetime` — `unwrap_or_default()` で epoch にフォールバック
- **Implications**: これらを全て `Result` 伝播に変えても `row_to_issue` の戻り型は変わらないため、呼び出し元の修正は不要

### str_to_state のエラー型選択

- **Context**: `str_to_state` を `Result` に変えるとき、何の `Error` 型を使うか
- **Sources Consulted**: rusqlite ドキュメント、既存コードの `rusqlite::Error` 使用箇所
- **Findings**:
  - `rusqlite::Error::InvalidColumnType(usize, String, rusqlite::types::Type)` が最も近い意味
  - ただし `FromSql` トレイトの実装として提供するわけではないため、`rusqlite::Error::ToSqlConversionFailure(Box<dyn Error + Send + Sync>)` か独自のエラーメッセージを含む `InvalidColumnType` のラッパーを使う選択肢もある
  - 最もシンプルな方法: `rusqlite::Error::InvalidColumnType(idx, value, rusqlite::types::Type::Text)` を構築して返す
- **Implications**: `row_to_issue` から見た呼び出し箇所は `str_to_state(&state_str)` のみで、エラー型を `rusqlite::Error` にすれば `?` でそのまま伝播できる

### fixing_causes JSON パースエラーの変換

- **Context**: `serde_json::Error` を `rusqlite::Error` に変換する方法
- **Findings**:
  - `rusqlite::Error::ToSqlConversionFailure(Box::new(e))` では意味が合わない
  - `rusqlite::Error::InvalidColumnType` は型不一致を表すが、ここではパース失敗なので微妙
  - 最も適切: `rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(e))` — SQLの値を Rust 型に変換するときの失敗を表す
- **Implications**: `rusqlite::Error::FromSqlConversionFailure` を使うことでエラーの意味が正確になる

### reset_for_restart の SQL 調査

- **Context**: `fixing_causes` が UPDATE 文から漏れているかの確認
- **Findings**:
  - L213-222: UPDATE文に `state`, `retry_count`, `current_pid`, `error_message`, `model`, `updated_at` は含まれるが `fixing_causes` が含まれない
  - `fixing_causes` は step4 で毎回 UPDATE されるため直接的な実害はないが、`reset_for_restart` の目的（完全なリセット）に反する
- **Implications**: `fixing_causes = '[]'` を UPDATE 文に追加するだけで修正完了

### sqlite_connection.rs のマイグレーション調査

- **Context**: `let _ = conn.execute_batch(...)` による `fixing_causes` マイグレーションの問題
- **Findings**:
  - L80-83: `feature_name`, `model` は `run_add_column_migration` 使用
  - L86-88: `fixing_causes` は `let _ = conn.execute_batch(...)` — エラーを全て無視
  - `run_add_column_migration` は "duplicate column name" エラーのみ無視し、それ以外は伝播する（冪等かつ安全）
- **Implications**: `fixing_causes` も同様に `run_add_column_migration` を使うよう変更すれば一貫性が保たれる

## Architecture Pattern Evaluation

| 選択肢 | 説明 | 利点 | リスク |
|--------|------|------|--------|
| 現状維持（サイレントフォールバック） | `unwrap_or_else` でデフォルト値を返す | シンプル | 破損データが検出されず予期しない動作を引き起こす |
| Result への変換（採用） | エラー時は `rusqlite::Error` を返し `?` で伝播 | 呼び出し元に変更なし、エラーログが出力される | なし |
| カスタムエラー型の導入 | `RepositoryError` 等を定義して意味を明確化 | 意味が明確 | 変更範囲が広がる、過剰設計 |

→ **採用**: Result への変換。呼び出し元の変更が不要で、最小限の変更で安全性が向上する。

## Design Decisions

### Decision: `str_to_state` の戻り型変更

- **Context**: `str_to_state` を `pub fn` から `fn` に変更し、戻り型を `Result<State, rusqlite::Error>` にする
- **Alternatives Considered**:
  1. `FromSql` トレイトを実装する — 変更量が増える
  2. `rusqlite::Error` のバリアントを直接構築する — 採用
- **Selected Approach**: `rusqlite::Error::InvalidColumnType(col_idx, value, rusqlite::types::Type::Text)` を返す
- **Rationale**: 既存の `rusqlite::Error` のみで完結し、外部依存を増やさない
- **Trade-offs**: エラーメッセージが `rusqlite` 内部形式になるが、ログに十分な情報が含まれる
- **Follow-up**: `pub` 可視性を落としてよいか確認（テストコードで使用されている場合は維持）

### Decision: `fixing_causes` JSON パースエラーの変換方法

- **Context**: `serde_json::Error` を `rusqlite::Error` に変換する
- **Selected Approach**: `rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(e))`
- **Rationale**: SQL カラムから Rust 型への変換失敗を正確に表現できる

## Risks & Mitigations

- **リスク**: 既存の SQLite データに破損した `fixing_causes` JSON が存在する場合、そのIssueが `find_active`/`find_by_state` でスキップされる
  - **緩和**: これが本修正の目的であり、破損Issueがサイレントに処理されるより明示的にエラーログへ記録される方が望ましい

- **リスク**: `str_to_state` が `pub` から `pub(crate)` または非公開に変わる場合、外部テストが壊れる
  - **緩和**: テストコード内の `str_to_state` 使用箇所を確認し、可視性を適切に設定する

## References

- rusqlite エラー型: `rusqlite::Error` enum（`FromSqlConversionFailure`, `InvalidColumnType` 等）
- 既存の実装パターン: `src/adapter/outbound/sqlite_connection.rs` の `run_add_column_migration`
