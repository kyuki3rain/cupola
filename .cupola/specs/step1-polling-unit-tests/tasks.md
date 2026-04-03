# Implementation Plan

## Task Format Template

- [ ] 1. step1 専用 mock 構造体の追加
- [ ] 1.1 MockGitHubStep1 を定義する
  - `list_ready_issues` / `is_issue_open` / `is_pr_merged` の戻り値をフィールドで制御できる mock 構造体を `#[cfg(test)] mod tests` 内に追加する
  - `ready_issues: Vec<GitHubIssue>` フィールドで返す Issue リストを指定できるようにする
  - `list_ready_issues_err: bool` フィールドで `Err` を返す動作を制御できるようにする
  - `issue_open: bool` フィールドで `is_issue_open` の戻り値を制御できるようにする
  - `is_issue_open_err: bool` フィールドで `is_issue_open` の `Err` を制御できるようにする
  - `pr_merged: bool` / `is_pr_merged_err: bool` フィールドで `is_pr_merged` の動作を制御できるようにする
  - その他の `GitHubClient` メソッドは `unimplemented!()` または Noop で実装する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4_

- [ ] 1.2 MockIssueRepoStep1 を定義する
  - `find_by_issue_number` の戻り値を issue 番号ごとに設定できる mock 構造体を `#[cfg(test)] mod tests` 内に追加する
  - `issues_by_number: HashMap<u64, Option<Issue>>` フィールドで issue 番号ごとの返却値を定義できるようにする
  - `active_issues: Vec<Issue>` フィールドで `find_active` が返す Issue リストを指定できるようにする
  - `find_active_err: bool` フィールドで `find_active` の `Err` 動作を制御できるようにする
  - その他の `IssueRepository` メソッドは `unimplemented!()` または Noop で実装する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3, 2.5_

- [ ] 2. 新規 Issue 検出のテストを実装する
- [ ] 2.1 agent:ready Issue が IssueDetected イベントを emit するテストを追加する
  - `list_ready_issues` が単一 Issue を返し、`find_by_issue_number` が `None` を返す場合に `events` に `IssueDetected` が 1 件 push されることを検証する
  - `#[tokio::test]` を使用した非同期テスト関数として実装する
  - _Requirements: 1.1_

- [ ] 2.2 複数 Issue が同時に検出された場合に各々が IssueDetected を emit するテストを追加する
  - `list_ready_issues` が複数 Issue を返した場合、`events` に各 Issue に対応する `IssueDetected` が積まれることを検証する
  - `#[tokio::test]` を使用した非同期テスト関数として実装する
  - _Requirements: 1.2_

- [ ] 2.3 非 terminal 状態の既存 Issue がスキップされるテストを追加する
  - `find_by_issue_number` が非 terminal 状態の Issue を返す場合、`events` に何も push されないことを検証する
  - _Requirements: 1.3_

- [ ] 2.4 terminal 状態の既存 Issue が再検出されるテストを追加する
  - `find_by_issue_number` が terminal 状態の Issue を返す場合、`events` に `IssueDetected` が push されることを検証する
  - _Requirements: 1.4_

- [ ] 2.5 list_ready_issues がエラーを返した場合にイベントが生成されないテストを追加する
  - `list_ready_issues` が `Err` を返す場合、`events` が空のままであることを検証する
  - _Requirements: 1.5_

- [ ] 3. close 検出のテストを実装する
- [ ] 3.1 active な Issue が close された場合に IssueClosed イベントが emit されるテストを追加する
  - `find_active` が非 ReviewWaiting 状態の active Issue を返し、`is_issue_open` が `false` を返す場合に `IssueClosed` が emit されることを検証する
  - _Requirements: 2.1, 2.2_

- [ ] 3.2 is_issue_open が true を返す場合にイベントが生成されないテストを追加する
  - `is_issue_open` が `true` を返す場合、`events` に何も push されないことを検証する
  - _Requirements: 2.3_

- [ ] 3.3 is_issue_open がエラーを返した場合にイベントが生成されないテストを追加する
  - `is_issue_open` が `Err` を返す場合、`events` が空のままであることを検証する
  - _Requirements: 2.4_

- [ ] 3.4 find_active がエラーを返した場合に close 検出処理がスキップされるテストを追加する
  - `find_active` が `Err` を返す場合、`events` が空のままであることを検証する
  - _Requirements: 2.5_

- [ ] 4. resolve_close_event_for_issue のテストを実装する
- [ ] 4.1 DesignReviewWaiting 状態で PR が merged の場合に DesignPrMerged を返すテストを追加する
  - `review_waiting_issue(State::DesignReviewWaiting, pr_num)` と `is_pr_merged` が `true` を返す mock を使用して `DesignPrMerged` が返ることを検証する
  - _Requirements: 3.1_

- [ ] 4.2 ImplementationReviewWaiting 状態で PR が merged の場合に ImplementationPrMerged を返すテストを追加する
  - `review_waiting_issue(State::ImplementationReviewWaiting, pr_num)` と `is_pr_merged` が `true` を返す mock を使用して `ImplementationPrMerged` が返ることを検証する
  - _Requirements: 3.2_

- [ ] 4.3 PR が未 merge の場合に IssueClosed を返すテストを追加する
  - `is_pr_merged` が `false` を返す mock を使用して `IssueClosed` が返ることを検証する
  - ReviewWaiting 状態の Issue で PR 番号が設定されている場合のテスト
  - _Requirements: 3.3_

- [ ] 4.4 is_pr_merged がエラーを返した場合に IssueClosed にフォールバックするテストを追加する
  - `is_pr_merged` が `Err` を返す mock を使用して `IssueClosed` が返ることを検証する
  - _Requirements: 3.4_

- [ ] 4.5 ReviewWaiting 以外の状態では常に IssueClosed を返すテストを追加する
  - `DesignRunning` や `ImplementationRunning` など非 ReviewWaiting 状態の Issue で `IssueClosed` が返ることを検証する
  - _Requirements: 3.5_

- [ ] 4.6 PR 番号が設定されていない場合に IssueClosed を返すテストを追加する
  - `design_pr_number` / `impl_pr_number` が `None` の ReviewWaiting 状態 Issue で `IssueClosed` が返ることを検証する
  - _Requirements: 3.6_

- [ ] 5. 品質チェックを実施する
- [ ] 5.1 全テストがパスすることを確認する
  - `cargo test` を実行してすべてのテスト（既存・新規）が pass することを確認する
  - テスト失敗がある場合はテストコードを修正する
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [ ] 5.2 静的解析・フォーマットチェックを通過させる
  - `RUSTFLAGS=-D warnings cargo clippy --all-targets` を実行して警告がないことを確認する
  - `cargo fmt -- --check` を実行してフォーマット準拠を確認する
  - 指摘がある場合は修正して再確認する
  - _Requirements: 4.4, 4.5_
