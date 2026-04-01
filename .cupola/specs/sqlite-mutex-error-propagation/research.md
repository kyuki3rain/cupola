# Research & Design Decisions

---
**Purpose**: sqlite-mutex-error-propagation フィーチャーの調査結果と設計判断の記録

---

## Summary
- **Feature**: `sqlite-mutex-error-propagation`
- **Discovery Scope**: Simple Addition（既存アダプターの一行置換、新コンポーネントなし）
- **Key Findings**:
  - 本番コードの `.expect("mutex poisoned")` は11箇所、すべて `anyhow::Result<T>` を返すメソッドの `spawn_blocking` closure 内に存在
  - `.await.expect("spawn_blocking panicked")` も各メソッドの末尾に存在（sqlite_execution_log_repository.rs: 3箇所、sqlite_issue_repository.rs: 8箇所）
  - すべてのメソッドは既存トレイトシグネチャ（`async fn ... -> anyhow::Result<T>`）を維持しているため、シグネチャ変更は不要

## Research Log

### 既存コードパターンの調査

- **Context**: 変更対象ファイルと `.expect()` 使用箇所を特定するために実施
- **Sources Consulted**: `sqlite_connection.rs`, `sqlite_execution_log_repository.rs`, `sqlite_issue_repository.rs`
- **Findings**:
  - `sqlite_connection.rs` L42: `init_schema()` 内の mutex lock（1箇所）
  - `sqlite_execution_log_repository.rs` L25, L48, L66: 各メソッド内の mutex lock（3箇所）
  - `sqlite_issue_repository.rs` L26, L46, L66, L86, L107, L139, L155, L188: 各メソッド内の mutex lock（8箇所）、合計11箇所
  - spawn_blocking の `.await.expect()` は execution_log で3箇所、issue_repository で8箇所
  - テストコードの `.expect()` は `sqlite_connection.rs` L124, L154, L188（変更不要）
- **Implications**: 全箇所が同一パターンで、機械的な置換で対応可能

### エラー型の調査

- **Context**: `map_err` で変換する型と既存エラー体系の整合性を確認
- **Sources Consulted**: `src/application/error.rs`
- **Findings**:
  - アプリケーション層に `CupolaError` enum が存在（`Database(#[from] rusqlite::Error)` を含む）
  - SQLite アダプターは `anyhow::Result` を使用（`CupolaError` ではない）
  - `PoisonError` は `anyhow::anyhow!()` で包むのが既存パターンと整合
- **Implications**: `map_err(|_| anyhow::anyhow!("..."))` で統一、型変換不要

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| map_err + ? | `PoisonError` を `anyhow::Error` に変換して伝搬 | 既存コードパターンと整合、シグネチャ変更不要 | なし | 採用 |
| カスタムエラー型 | `MutexPoisonedError` を定義して返す | 型情報が明確 | シグネチャ変更が必要、over-engineering | 不採用 |
| unwrap_or_else | デフォルト値を返す | パニック回避 | ビジネスロジック的に不正な状態 | 不採用 |

## Design Decisions

### Decision: `anyhow::anyhow!()` による一律エラーラップ

- **Context**: `mutex.lock()` の失敗をどのエラー型で返すか
- **Alternatives Considered**:
  1. `anyhow::anyhow!("failed to acquire database lock")` — 文字列メッセージで包む
  2. `CupolaError::Database` に変換する — アプリ層エラー型を使う
- **Selected Approach**: `anyhow::anyhow!("failed to acquire database lock")` を採用
- **Rationale**: 既存アダプターが `anyhow::Result` を返しており、`CupolaError` への変換は不要。Issue に示された修正方針と整合
- **Trade-offs**: エラー型の詳細は失われるが、ログに十分なメッセージが残る
- **Follow-up**: エラーメッセージの統一性（"failed to acquire database lock"）を全箇所で確認

### Decision: spawn_blocking エラーのメッセージ形式

- **Context**: `JoinError` を `anyhow::Error` に変換する際のメッセージ形式
- **Alternatives Considered**:
  1. `.await.map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?` — エラー内容を含む
  2. `.await.map_err(|_| anyhow::anyhow!("spawn_blocking panicked"))` — 固定メッセージ
- **Selected Approach**: `{e}` でエラー内容を含める形式を採用
- **Rationale**: デバッグ時に有用な情報が得られる。Issue の修正方針と整合
- **Trade-offs**: なし

## Risks & Mitigations

- mutex 毒化は通常発生しないが、前の lock 保持中にパニックした場合にのみ起こる — `map_err` で伝搬するのみで追加的なリカバリ処理は不要
- テストコード変更の誤適用リスク — `#[cfg(test)]` ブロック内を対象外と明示することで回避

## References

- Rust std::sync::Mutex::lock — PoisonError の説明（標準ライブラリドキュメント）
- anyhow crate — `anyhow::anyhow!()` マクロの使用方法
