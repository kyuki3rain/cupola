# Research & Design Decisions

---
**Purpose**: `step1_issue_polling` および `resolve_close_event_for_issue` のユニットテスト追加に向けた調査・設計決定の記録。

---

## Summary
- **Feature**: `step1-polling-unit-tests`
- **Discovery Scope**: Extension（既存コードへのテスト追加）
- **Key Findings**:
  - 既存 `#[cfg(test)] mod tests` ブロックに `MockGitHub`・`MockIssueRepo`・`NoopExecLogRepo`・`NoopClaudeRunner`・`NoopWorktree` の完全な mock 定義が存在し、step4/step7 テストで実績がある。
  - `step1_issue_polling` は `&self`（不変参照）を受け取り、`events: &mut Vec<(i64, Event)>` に結果を追記する設計のため、テスト側で events ベクタを用意して呼び出すだけで検証可能。
  - `resolve_close_event_for_issue` も `&self` + `&Issue` → `Event` という純粋な非同期関数であり、mock の `is_pr_merged` 戻り値を制御するだけで全分岐をテストできる。

## Research Log

### 既存 mock 定義の調査
- **Context**: 新規 mock を最小限にするため、既存テストの mock 定義を調査した。
- **Sources Consulted**: `src/application/polling_use_case.rs` の `#[cfg(test)] mod tests`（行 1121-）
- **Findings**:
  - `MockGitHub` は `list_ready_issues` / `is_issue_open` が `unimplemented!()` であるため、step1 テスト用に個別の mock 構造体（または制御フィールド追加）が必要。
  - `MockIssueRepo` は `find_by_issue_number` が `unimplemented!()` であるため、同様に拡張が必要。
  - `make_uc` / `review_waiting_issue` などのヘルパー関数は再利用可能。
- **Implications**:
  - step1 専用の `MockGitHubStep1` と `MockIssueRepoStep1` を追加する、または既存 `MockGitHub` に `list_ready_issues` / `is_issue_open` フィールドを追加するアプローチを選択する必要がある。
  - 既存テストへの影響を最小化するため、step1 専用の別 mock 構造体を追加する方針を採用する。

### step1_issue_polling のロジック分析
- **Context**: テストすべき分岐を網羅するためにロジックを精査した。
- **Sources Consulted**: `src/application/polling_use_case.rs` 行 148-204
- **Findings**:
  - 新規 Issue 検出: `list_ready_issues` → 各 Issue に対して `find_by_issue_number` → 非 terminal なら skip、terminal / 未登録なら `IssueDetected` push
  - close 検出: `find_active` → 各 Issue に対して `is_issue_open` → false なら `resolve_close_event_for_issue` → events push
  - エラーケース: 各 API 呼び出しが `Err` の場合は `tracing::warn!` して処理継続（イベントは push しない）
- **Implications**: 各分岐を独立したテストケースにマッピングできる。

### resolve_close_event_for_issue のロジック分析
- **Context**: `ReviewWaiting` 系状態での PR merge 判定ロジックを確認した。
- **Sources Consulted**: `src/application/polling_use_case.rs` 行 970-1003
- **Findings**:
  - `DesignReviewWaiting` → design_pr_number を確認 → merged なら `DesignPrMerged`
  - `ImplementationReviewWaiting` → impl_pr_number を確認 → merged なら `ImplementationPrMerged`
  - PR 番号が None → `IssueClosed`
  - `is_pr_merged` がエラー → `IssueClosed`（フォールバック）
  - 他の状態 → 即 `IssueClosed`
- **Implications**: 6つのテストケースで全分岐をカバーできる。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 既存 mock 拡張 | `MockGitHub` に `list_ready_issues` / `is_issue_open` フィールドを追加 | mock 構造体の統一 | 既存テストの init コードへの影響が大きい | 採用しない |
| step1 専用 mock | step1 テスト用に別の mock 構造体を定義 | 既存テストへの影響ゼロ・テストごとに制御しやすい | 小規模なコード重複 | **採用** |

## Design Decisions

### Decision: step1 専用 mock 構造体の追加
- **Context**: 既存 `MockGitHub` は `list_ready_issues` / `is_issue_open` が `unimplemented!()` であり、step1 テストに流用できない。
- **Alternatives Considered**:
  1. 既存 `MockGitHub` にフィールドを追加して全メソッドに対応させる
  2. step1 専用の `MockGitHubStep1` / `MockIssueRepoStep1` を定義する
- **Selected Approach**: Option 2（専用 mock の追加）
- **Rationale**: 既存テストへの変更を最小化し、テスト間の独立性を保つ。step1 のテストは step4/step7 とは異なる mock 動作が必要。
- **Trade-offs**: 少量のコード重複が生じるが、保守性・可読性は向上する。
- **Follow-up**: 将来的に mock を共通化するリファクタリングは別 Issue で対応する。

### Decision: `step1_issue_polling` を直接テストする
- **Context**: `step1_issue_polling` は private な非同期メソッドだが、`mod tests` が同一ファイル内にあるため `use super::*` でアクセス可能。
- **Selected Approach**: `uc.step1_issue_polling(&mut events).await` を直接呼び出す。
- **Rationale**: `run_cycle` 全体をテストすると他のステップとの依存が生じ、テストが複雑になる。step1 の独立したテストがシンプルで保守しやすい。

## Risks & Mitigations
- `MockGitHub` の `unimplemented!()` メソッドを step1 テストで誤って呼び出した場合にパニック → 各テストケースで必要なメソッドのみを実装した専用 mock を使用することで回避
- Rust の `async fn in trait` は Edition 2024 で安定しているが、mock 実装でのコンパイルエラーリスク → 既存の mock 実装パターンに倣う

## References
- `src/application/polling_use_case.rs` — テスト対象の実装コード
- `src/application/port/github_client.rs` — `GitHubClient` trait 定義
- `src/application/port/issue_repository.rs` — `IssueRepository` trait 定義
- `src/domain/state.rs` — `State` enum と `is_terminal()` / `is_active()` メソッド
