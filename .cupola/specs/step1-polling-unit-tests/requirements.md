# Requirements Document

## Project Description (Input)
polling_use_case の step1（Issue 検出・close 検出）にユニットテストが存在しない問題を解決するため、新規 Issue 検出・close 検出・resolve_close_event_for_issue の各シナリオをカバーするユニットテストを追加する

## Introduction
`src/application/polling_use_case.rs` の `step1_issue_polling` 関数および `resolve_close_event_for_issue` 関数は、Cupola エージェントのコアとなる Issue 検出・close 検出ロジックを担うが、現時点でユニットテストが存在しない。本仕様では、既存の mock 定義を再利用しながら、各シナリオをカバーするユニットテストを `#[cfg(test)] mod tests` ブロックに追加することを目的とする。

## Requirements

### Requirement 1: 新規 Issue 検出テスト

**Objective:** テストエンジニアとして、`step1_issue_polling` における新規 Issue 検出ロジックをユニットテストしたい。これにより、`agent:ready` ラベル付き Issue が正しく検出され `IssueDetected` イベントが生成されることを保証できる。

#### Acceptance Criteria
1. When `list_ready_issues` が `agent:ready` ラベル付きの Issue リストを返し、かつ DB に該当 Issue が存在しない場合、the PollingUseCase shall `IssueDetected { issue_number }` イベントをイベントリストに追加する。
2. When `list_ready_issues` が複数の Issue を返す場合、the PollingUseCase shall 各 Issue に対して個別の `IssueDetected` イベントをイベントリストに追加する。
3. While DB に該当 Issue が存在し、かつその Issue が非 terminal 状態にある場合、the PollingUseCase shall `IssueDetected` イベントをイベントリストに追加しない（重複検出を防ぐ）。
4. When DB に terminal 状態の Issue が存在する場合、the PollingUseCase shall 該当 Issue に対して `IssueDetected` イベントをイベントリストに追加する（再処理を許可する）。
5. If `list_ready_issues` がエラーを返す場合、the PollingUseCase shall イベントリストに何も追加せず処理を継続する。

### Requirement 2: Issue close 検出テスト

**Objective:** テストエンジニアとして、`step1_issue_polling` における Issue close 検出ロジックをユニットテストしたい。これにより、active な Issue が close された場合に正しいイベントが生成されることを保証できる。

#### Acceptance Criteria
1. When `find_active` が active な Issue を返し、かつ `is_issue_open` が `false` を返す場合、the PollingUseCase shall `IssueClosed` または状態に応じた merge イベントをイベントリストに追加する。
2. When active な Issue が close され、かつその Issue が `DesignReviewWaiting` または `ImplementationReviewWaiting` 以外の状態にある場合、the PollingUseCase shall `IssueClosed` イベントをイベントリストに追加する。
3. While `is_issue_open` が `true` を返す場合、the PollingUseCase shall 該当 Issue に対してイベントをイベントリストに追加しない。
4. If `is_issue_open` がエラーを返す場合、the PollingUseCase shall 該当 Issue のイベントをスキップし処理を継続する。
5. If `find_active` がエラーを返す場合、the PollingUseCase shall close 検出処理をスキップし処理を継続する。

### Requirement 3: resolve_close_event_for_issue テスト

**Objective:** テストエンジニアとして、`resolve_close_event_for_issue` のロジックをユニットテストしたい。これにより、PR merge 状態に応じて正しいイベントが選択されることを保証できる。

#### Acceptance Criteria
1. When Issue が `DesignReviewWaiting` 状態にあり、かつ関連する design PR が merged の場合、the PollingUseCase shall `DesignPrMerged` イベントを返す。
2. When Issue が `ImplementationReviewWaiting` 状態にあり、かつ関連する implementation PR が merged の場合、the PollingUseCase shall `ImplementationPrMerged` イベントを返す。
3. When Issue が `ReviewWaiting` 系の状態にあり、かつ関連 PR が未 merge の場合、the PollingUseCase shall `IssueClosed` イベントを返す。
4. If `is_pr_merged` が API エラーを返す場合、the PollingUseCase shall `IssueClosed` イベントにフォールバックする。
5. When Issue が `DesignReviewWaiting` / `ImplementationReviewWaiting` 以外の状態にある場合、the PollingUseCase shall PR の確認なしに `IssueClosed` イベントを返す。
6. When Issue が `ReviewWaiting` 系の状態にあるが PR 番号が設定されていない場合、the PollingUseCase shall `IssueClosed` イベントを返す。

### Requirement 4: テストコードの品質要件

**Objective:** テストエンジニアとして、既存の mock 定義を再利用し保守性の高いテストコードを実装したい。これにより、テストの冗長性を排除し将来の変更に対して安定したテストスイートを維持できる。

#### Acceptance Criteria
1. The テストコード shall `polling_use_case.rs` 内の既存 `#[cfg(test)] mod tests` ブロックに追加される。
2. The テストコード shall 既存の mock 実装（`MockGitHubClient`、`MockIssueRepository` 等）を再利用する。
3. The テストコード shall `#[tokio::test]` を使用した非同期テスト関数として実装される。
4. The テストコード shall `cargo test` および `cargo clippy -- -D warnings` をすべてパスする。
5. The テストコード shall step4・step7 の既存テストと一貫したスタイル・構造で記述される。
