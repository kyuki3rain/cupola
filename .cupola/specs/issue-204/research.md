# Research & Design Decisions

---
**Purpose**: 本ドキュメントは、`observe_pr_for_type` の堅牢化に関する調査結果と設計判断の根拠を記録する。

---

## Summary

- **Feature**: `find_latest_with_pr_number` — SQLレベルでの `pr_number IS NOT NULL` フィルタ強制
- **Discovery Scope**: Extension（既存システムへの拡張）
- **Key Findings**:
  - 既存の `ProcessRunRepository` トレイトは同パターン (`find_latest`, `find_latest_with_consecutive_count`) を持ち、新メソッドはその延長として自然に追加できる
  - `SqliteProcessRunRepository` は `spawn_blocking` + `Arc<Mutex<Connection>>` パターンを一貫して使用しており、新実装もこれに従う
  - アーキテクチャ仕様 (`docs/architecture/observations.md`) は `pr_number IS NOT NULL` を明示的なフィルタ条件として規定しており、現実装は仕様違反である

## Research Log

### 既存の ProcessRunRepository パターン分析

- **Context**: 新メソッド追加前に、既存トレイトのパターンを把握する必要があった
- **Sources Consulted**: `src/application/port/process_run_repository.rs`, `src/adapter/outbound/sqlite_process_run_repository.rs`
- **Findings**:
  - `find_latest(issue_id, type_)` が既に存在し、新メソッドは同シグネチャで `pr_number IS NOT NULL` を追加するだけ
  - `find_latest_with_consecutive_count` のように複合クエリのパターンも存在する
  - 全ての非同期メソッドは `impl std::future::Future<Output = Result<...>> + Send` を返す（Rust 2024 Edition 対応）
- **Implications**: `find_latest_with_pr_number` は同パターンで定義でき、呼び出し側の挙動は大きく変えずに導入できる。ただし `ProcessRunRepository` トレイトに追加する場合、`SqliteProcessRunRepository` やテスト用モックなど既存のトレイト実装は更新が必要になる

### SQLite クエリパターン分析

- **Context**: SQLite アダプターの実装パターンを確認
- **Sources Consulted**: `src/adapter/outbound/sqlite_process_run_repository.rs` (特に `find_latest` 実装)
- **Findings**:
  - 全クエリは `tokio::task::spawn_blocking` でラップされ、`Arc<Mutex<Connection>>` を通じてアクセス
  - `find_latest` の SQL: `WHERE issue_id = ?1 AND type = ?2 ORDER BY id DESC LIMIT 1`
  - 新クエリは同 SQL に `AND pr_number IS NOT NULL` を追加するだけ
- **Implications**: 既存のヘルパー関数（`row_to_process_run` 等）を再利用でき、最小限の変更で実装可能

### アーキテクチャ仕様との整合性確認

- **Context**: 修正が仕様に準拠していることを確認
- **Sources Consulted**: `docs/architecture/observations.md:16-17`
- **Findings**:
  - 仕様は `design_pr` を「`type=design` かつ `pr_number IS NOT NULL` の最新レコード」と定義
  - 現実装はこの条件をRust側でフィルタしており仕様違反
  - 新実装はSQLで強制することで仕様に完全準拠
- **Implications**: 本修正は機能追加ではなく仕様違反の修正であり、外部から期待される振る舞いは維持される。ただし、`ProcessRunRepository` には新メソッド追加が必要なため、トレイト実装者やモックには追随修正が発生する

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 新メソッド追加 | `find_latest_with_pr_number` をトレイトに追加 | 単一責任、型安全、仕様準拠 | トレイト変更はモック更新が必要 | 採用 |
| `find_latest` の変更 | 既存メソッドにオプション引数を追加 | 変更箇所が少ない | 既存呼び出し箇所への影響、後方互換性の問題 | 非採用 |
| アプリケーション層でのフィルタ改善 | Rustコードを修正して複数行を試みる | DB変更不要 | 複数クエリ、N+1問題のリスク | 非採用 |

## Design Decisions

### Decision: 既存の `find_latest` は変更せず新メソッドを追加する

- **Context**: `find_latest` は他の呼び出し箇所 (`collect.rs` 以外) でも使用されており、変更すると影響範囲が広がる
- **Alternatives Considered**:
  1. `find_latest` にフラグ引数を追加 — 既存呼び出しへの影響と後方互換性の問題
  2. `find_latest` を新しい動作に変更 — 他の呼び出し箇所の意図を変えてしまうリスク
  3. 新メソッドを独立して追加（採用）— 最小変更で責任を明確に分離
- **Selected Approach**: `find_latest_with_pr_number` を新しいトレイトメソッドとして追加し、`observe_pr_for_type` のみがこれを使用する
- **Rationale**: クリーンアーキテクチャの原則に従い、ポートは使用側の要件を明確に表現すべき。`pr_number IS NOT NULL` という制約はポートの契約として表現するのが適切
- **Trade-offs**: トレイトのモック実装の更新が必要だが、テストの意図が明確になる
- **Follow-up**: 既存の `find_latest` 呼び出しが引き続き正しく動作することを回帰テストで確認

## Risks & Mitigations

- **トレイトモック更新漏れ**: テスト内のモック `ProcessRunRepository` 実装に `find_latest_with_pr_number` を追加し忘れるリスク — コンパイルエラーで確実に検出される（Rustの型システムが保護）
- **既存テストの回帰**: `find_latest` を引き続き使用する箇所が誤動作するリスク — 変更は `observe_pr_for_type` の1箇所のみであり影響が限定的
- **Cargo clippy 警告**: 新メソッドのシグネチャがプロジェクトの Clippy 設定 (`expect_used = "deny"`) に違反するリスク — 実装で `.expect()` を使わず `.context()` を使用することで回避

## References

- `src/application/port/process_run_repository.rs` — 既存の `ProcessRunRepository` トレイト定義
- `src/adapter/outbound/sqlite_process_run_repository.rs` — SQLite アダプター実装パターン
- `src/application/polling/collect.rs` — `observe_pr_for_type` の現在の実装
- `docs/architecture/observations.md` — アーキテクチャ仕様（`pr_number IS NOT NULL` の明示的要件）
