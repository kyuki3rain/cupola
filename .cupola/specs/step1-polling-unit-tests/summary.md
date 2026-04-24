# step1-polling-unit-tests サマリ

## Feature
`polling_use_case.rs` の `step1_issue_polling` と `resolve_close_event_for_issue` に対するユニットテストを追加。テスト未整備だった Cupola の Issue 検出・close 検出・PR merge 判定という中核ロジックに対し、step4/step7 と一貫したスタイルでカバレッジを確立する。プロダクションコードは一切変更しない。

## 要件サマリ
- **新規 Issue 検出 (Req 1)**: `agent:ready` 単一/複数 Issue → `IssueDetected`、非 terminal 既存 Issue はスキップ、terminal 既存 Issue は再検出、`list_ready_issues` エラー時は events 空。
- **close 検出 (Req 2)**: active かつ `is_issue_open=false` で非 ReviewWaiting なら `IssueClosed`、`is_issue_open=true` で events 空、`is_issue_open`/`find_active` エラーでスキップ継続。
- **`resolve_close_event_for_issue` (Req 3)**: `DesignReviewWaiting` + PR merged → `DesignPrMerged`、`ImplementationReviewWaiting` + PR merged → `ImplementationPrMerged`、PR 未 merge → `IssueClosed`、`is_pr_merged` エラー → `IssueClosed` フォールバック、非 ReviewWaiting 状態 → 即 `IssueClosed`、PR 番号 `None` → `IssueClosed`。
- **品質要件 (Req 4)**: 既存 `#[cfg(test)] mod tests` に追加、既存 mock/ヘルパー再利用、`#[tokio::test]` 非同期、`cargo test` + `cargo clippy -- -D warnings` 全パス、step4/step7 と一貫したスタイル。

## アーキテクチャ決定
- **step1 専用 mock `MockGitHubStep1` / `MockIssueRepoStep1` を新規追加** (採用): 既存 `MockGitHub` / `MockIssueRepo` は `list_ready_issues` / `is_issue_open` / `find_by_issue_number` が `unimplemented!()` で step1 に流用不可。既存 mock 拡張案は init コードへの波及が大きいため不採用。テスト間の独立性を重視し、少量のコード重複を許容。
- **`step1_issue_polling` を `run_cycle` 経由せず直接呼び出す**: `&self` 不変参照 + `events: &mut Vec<(i64, Event)>` 追記設計のため、`uc.step1_issue_polling(&mut events).await` で単独テスト可能。`run_cycle` 経由は他ステップとの依存でテストが複雑化。
- **`#[cfg(test)] mod tests` 同一ファイル配置**: `use super::*` で private メソッド `step1_issue_polling` / `resolve_close_event_for_issue` にアクセスできるため。
- **制御はフィールド + `*_err: bool` フラグパターン**: 各 mock が戻り値 + エラー発生の両方を制御可能に。
- **`MockIssueRepoStep1` は `HashMap<u64, Option<Issue>>` で issue 番号ごとに返却値指定**: 複数 Issue シナリオで個別制御が必要なため。
- **Noop 系 (`NoopExecLogRepo` / `NoopClaudeRunner` / `NoopWorktree`) と `make_uc` / `review_waiting_issue` ヘルパーは再利用**: 重複を避け保守性を高める。

## コンポーネント
- `src/application/polling_use_case.rs` `#[cfg(test)] mod tests` に以下を追加:
  - `MockGitHubStep1`: `ready_issues`, `list_ready_issues_err`, `issue_open`, `is_issue_open_err`, `pr_merged`, `is_pr_merged_err` フィールド。他メソッドは `unimplemented!()`。
  - `MockIssueRepoStep1`: `issues_by_number: HashMap<u64, Option<Issue>>`, `active_issues`, `find_active_err`。他メソッドは `unimplemented!()`。
  - 新規 Issue 検出テスト 5 件 (`step1_new_issue_detected_emits_issue_detected_event` など)。
  - close 検出テスト 4 件 (`step1_closed_non_review_waiting_emits_issue_closed` など)。
  - `resolve_close_event_for_issue` テスト 6 件 (`resolve_close_design_review_waiting_merged_emits_design_pr_merged` など)。
- プロダクションコード (`step1_issue_polling` / `resolve_close_event_for_issue`) は変更なし。

## 主要インターフェース
```rust
struct MockGitHubStep1 {
    ready_issues: Vec<GitHubIssue>,
    list_ready_issues_err: bool,
    issue_open: bool,
    is_issue_open_err: bool,
    pr_merged: bool,
    is_pr_merged_err: bool,
}
struct MockIssueRepoStep1 {
    issues_by_number: HashMap<u64, Option<Issue>>,
    active_issues: Vec<Issue>,
    find_active_err: bool,
}
```

## 学び / トレードオフ
- 既存 mock を拡張するか専用 mock を追加するかは「既存テストへの影響」vs「コード重複」のトレードオフ。本件は前者を重視して専用 mock を採用し、将来の mock 統一リファクタリングは別 Issue に切り出す方針。
- `step1_issue_polling` が `&self` + 外部 `events` vec という純粋な追記設計だったため、`run_cycle` 全体を動かさずに単独検証が可能で、テスト設計が大幅にシンプルになった。既存プロダクションコードの設計が testability を担保していた好例。
- エラーケース（API 失敗）は events が空のままであることを検証する消極的アサートだが、`tracing::warn!` のログ出力は今回は検証対象外とし、panic しない挙動の保証にとどめた。
