# Research & Design Decisions

---
**Purpose**: 本ドキュメントは、`check_label_actor` 最適化の調査結果と設計判断を記録する。

---

## Summary

- **Feature**: `check_label_actor` の Idle 限定呼び出し最適化
- **Discovery Scope**: Simple Addition（既存関数のシグネチャ拡張 + 条件分岐追加）
- **Key Findings**:
  - `ready_label_trusted` を参照する箇所は `decide_idle` のみであり、non-Idle 状態での `check_label_actor` 呼び出しは純粋に無駄
  - `observe_github_issue` は `issue.state` を受け取っていないため、呼び出し元（`observe_issue`）から渡す変更が必要
  - Cancelled → Idle 遷移は `decide` フェーズで行われるため、`collect` フェーズが再度走る次サイクルでは `state == Idle` となっており問題なし

## Research Log

### `ready_label_trusted` の消費箇所

- **Context**: non-Idle 状態でのスキップが安全かどうかを確認するため、`ready_label_trusted` の参照箇所を調査した
- **Sources Consulted**: `src/domain/decide.rs`
- **Findings**:
  - `decide_idle` のみが `ready_label_trusted` を参照する
  - `decide_initialize_running` 以降の状態遷移関数はいずれも `ready_label_trusted` を参照しない
  - `decide_cancelled` も `ready_label_trusted` を参照しない
- **Implications**: non-Idle 状態で `ready_label_trusted: false` を設定しても、動作に影響はない

### `observe_github_issue` 呼び出しフロー

- **Context**: `issue.state` を `observe_github_issue` に渡す方法を確認するため、呼び出し連鎖を調査した
- **Sources Consulted**: `src/application/polling/collect.rs`
- **Findings**:
  - 呼び出し連鎖: `collect_all` → `observe_issue` → `observe_github_issue`
  - `observe_issue` は `issue: &Issue` を受け取っており、`issue.state` を参照可能
  - `observe_github_issue` は `issue_number`, `is_open`, `labels` のみを受け取っており、`state` を受け取っていない
- **Implications**: `observe_github_issue` のシグネチャに `issue_state: State` を追加し、`observe_issue` から `issue.state` を渡す

### Cancelled → Idle 遷移のサイクルタイミング

- **Context**: Cancelled 状態から Idle に遷移した直後のサイクルで `check_label_actor` が必要かを確認した
- **Sources Consulted**: `src/application/polling/collect.rs`, `src/domain/decide.rs`
- **Findings**:
  - `collect` フェーズは DB の `issue.state` を読み取る
  - `decide` フェーズで Cancelled → Idle 遷移が起きた場合、DB 書き込み後に次のポーリングサイクルが開始する
  - 次サイクルの `collect` フェーズでは `state == Idle` として読み取られる
- **Implications**: Cancelled → Idle 遷移の同サイクルでは `state == Cancelled` のため `check_label_actor` はスキップされるが、次サイクルで `state == Idle` として正しく呼ばれる。動作上の問題はない

### 既存テストの構造

- **Context**: 変更に伴うテスト更新範囲を確認した
- **Sources Consulted**: `src/application/polling/collect.rs` (tests モジュール)
- **Findings**:
  - `observe_github_issue` への直接テストが3件ある
  - `closed_issue_returns_closed_snapshot`: `is_open=false` → API 非呼び出し
  - `open_issue_with_ready_label_calls_permission_api`: `is_open=true, ready_label=true` → API 呼び出し
  - `open_issue_without_ready_label_is_not_trusted`: `is_open=true, ready_label=false` → API 非呼び出し
  - 現在のテストは `issue_state` 引数を持たないため、更新が必要
- **Implications**: 既存3件のテストは `State::Idle` を前提とするよう更新が必要。新たに非 Idle 状態のテストを追加する

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| シグネチャ拡張（採用） | `observe_github_issue` に `issue_state: State` を追加 | シンプル、既存コードへの影響最小 | なし | 呼び出し元は既に `issue` を保持 |
| `Issue` 全体を渡す | `observe_github_issue` に `issue: &Issue` を渡す | 将来の拡張に備えられる | 過剰な依存、現在不要なフィールドも渡す | YAGNI 原則に反する |
| `WorldSnapshot` フラグ | `non_idle` フラグを `WorldSnapshot` に追加 | なし | 設計の一貫性を損なう | 不採用 |

## Design Decisions

### Decision: `issue_state: State` をシグネチャに追加する

- **Context**: `observe_github_issue` が Idle 状態かどうかを判断するために state 情報が必要
- **Alternatives Considered**:
  1. `issue: &Issue` を渡す — 過剰な依存
  2. `issue_state: State` を渡す — 最小限の変更
- **Selected Approach**: `issue_state: State` を追加し、`issue_state == State::Idle` かつ `has_ready_label == true` の場合のみ `check_label_actor` を呼ぶ
- **Rationale**: 変更範囲を最小化し、既存の関数シグネチャの意図を明確に保つ
- **Trade-offs**: 将来 state 以外の条件が追加された場合は再度シグネチャを変更する必要があるが、現時点では適切
- **Follow-up**: `State::all()` を使ったパラメータ化テストで全 variant を網羅する

## Risks & Mitigations

- Cancelled → Idle 遷移直後のサイクルで `ready_label` が信頼されない問題 — 次サイクルで `state == Idle` となるため実質影響なし（設計上の意図通り）
- `check_label_actor` スキップにより `RejectUntrustedReadyIssue` エフェクトが発火しない — non-Idle 状態では `decide_idle` 自体が呼ばれないため問題なし

## References

- `src/application/polling/collect.rs` — 対象ファイル
- `src/domain/decide.rs` — `ready_label_trusted` の消費箇所
- `src/domain/state.rs` — `State` enum 定義
- `src/application/association_guard.rs` — `check_label_actor` 実装
