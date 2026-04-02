# Implementation Plan

- [x] 1. ポートトレイトの拡張
- [x] 1.1 IssueRepository に Cancelled 状態の Issue を取得するメソッドを追加する
  - `IssueRepository` トレイトに `find_by_state(state: State) -> impl Future<Output = Result<Vec<Issue>>> + Send` を定義する
  - cleanup コマンドが Cancelled Issue を取得するための基盤となる
  - テスト用モックアダプターにも同メソッドを追加する（統合テストファイル内）
  - _Requirements: 5.1_

- [x] 1.2 (P) GitHubClient に PR の三状態を取得するメソッドを追加する
  - `PrStatus { Open, Closed, Merged }` 列挙型を `github_client.rs` に定義する
  - `GitHubClient` トレイトに `get_pr_status(&self, pr_number: u64) -> impl Future<Output = Result<PrStatus>> + Send` を定義する
  - テスト用モックアダプターにも同メソッドを追加する
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_

- [x] 2. アダプター実装の更新
- [x] 2.1 reset_for_restart の SQL をフィールド保持型に修正する
  - `SqliteIssueRepository::reset_for_restart` の UPDATE 文から `design_pr_number`, `impl_pr_number`, `worktree_path`, `feature_name` の NULL 化を削除する
  - `state='idle'`, `retry_count=0`, `current_pid=NULL`, `error_message=NULL`, `model=NULL` をリセットする
  - _Requirements: 1.1, 1.2_

- [x] 2.2 SqliteIssueRepository に find_by_state を実装する
  - Task 1.1 のポート定義を実装する
  - `SELECT * FROM issues WHERE state = ?1` の SQL で指定した state の Issue を全件取得する
  - state 文字列のシリアライズは既存の `update_state` と同じパターンを踏襲する
  - _Requirements: 5.1_

- [x] 2.3 (P) GitHubClientImpl に get_pr_status を実装する
  - Task 1.2 のポート定義を実装する
  - GitHub REST API で対象 PR の `merged` と `state` フィールドを取得する
  - `merged == true` → `PrStatus::Merged`、`state == "open"` → `PrStatus::Open`、それ以外 → `PrStatus::Closed` としてマッピングする
  - 2.1, 2.2 とは別ファイルを変更するため並行実行可能
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_

- [x] 3. (P) TransitionUseCase から RetryExhausted 時の cleanup を削除する
  - `execute_side_effects` の `(State::Cancelled, Event::RetryExhausted)` アームで呼び出している `self.cleanup(issue).await` を削除する
  - GitHub Issue の close 処理は引き続き実行する
  - `(State::Cancelled, Event::IssueClosed)` アームの cleanup は変更しない（人間が意図的に close した場合は cleanup を実行し続ける）
  - Task 1, 2 とは独立して実装可能
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 4. (P) i18n に再開メッセージキーを追加する
  - `locales/en.yml` に `issue_comment.resuming_design: "Resuming design"` を追加する
  - `locales/ja.yml` に `issue_comment.resuming_design: "設計を再開します"` を追加する
  - `locales/en.yml` に `issue_comment.resuming_implementation: "Resuming implementation"` を追加する
  - `locales/ja.yml` に `issue_comment.resuming_implementation: "実装を再開します"` を追加する
  - ほかのタスクとは独立して実装可能
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [x] 5. CleanupUseCase を新規実装する
- [x] 5.1 CleanupUseCase の実装本体を作成する
  - `src/application/cleanup_use_case.rs` を新規作成し、`CleanupUseCase<I: IssueRepository, W: GitWorktree>` 構造体を定義する
  - `execute() -> Result<CleanupResult>` メソッドを実装する
  - Cancelled Issue を全件取得し、worktree の削除 → ブランチ削除 → DB フィールド NULL 化の順で処理する
  - worktree・ブランチが存在しない場合はスキップし、エラーはログに記録して次の Issue の処理を継続する
  - `CleanupResult` と `CleanedIssue` 型を定義し、削除した worktree・ブランチ名を保持する
  - Task 1.1, 2.2 が完了していることが前提（find_by_state の port と実装）
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6_

- [x] 5.2 CleanupUseCase の実行結果を標準出力に表示する機能を追加する
  - `CleanupResult` の内容を整形して標準出力に表示するロジックを実装する
  - 削除した worktree パスとブランチ名を Issue 番号とともに表示する
  - 削除対象がなかった場合も「対象の Cancelled Issue が見つかりませんでした」のように表示する
  - _Requirements: 5.5_

- [x] 6. PollingUseCase を修正する
- [x] 6.1 initialize_issue を冪等化する
  - worktree のパスを構築した後に `Path::exists()` で存在確認を行い、分岐する
  - worktree が存在しない場合: 従来通り worktree を作成し、`design_starting` メッセージを投稿する
  - worktree が存在する場合: 作成をスキップし、Issue の状態（DesignRunning/ImplementationRunning）に応じて `resuming_design` または `resuming_implementation` メッセージを投稿する
  - 既存 worktree の場合、`issue.worktree_path` は DB に既にセットされているため `issue_repo.update()` のスキップまたは冪等な更新にする
  - Task 4 の i18n キー追加が前提
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 6.5, 6.6_

- [x] 6.2 step7_spawn_processes にスポーン前 PR 状態チェックを追加する
  - `DesignRunning` 状態の Issue でスポーンしようとする前に `issue.design_pr_number` を確認する
  - `ImplementationRunning` 状態の Issue でスポーンしようとする前に `issue.impl_pr_number` を確認する
  - PR 番号が存在する場合は `get_pr_status` で状態を取得し、`Merged` なら対応する merge イベント（DesignPrMerged / ImplementationPrMerged）を events にプッシュしてスキップ、`Open` なら `ProcessCompletedWithPr` を events にプッシュしてスキップ、`Closed` または API エラーなら通常通りスポーンする
  - スキップ判断はログに記録する
  - Task 1.2, 2.3 が完了していることが前提（get_pr_status の port と実装）
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_

- [x] 7. CLI と bootstrap を統合する
- [x] 7.1 CLI に Cleanup サブコマンドを追加する
  - `Command` enum に `Cleanup { config: PathBuf }` バリアントを追加する
  - `--config` フラグのデフォルト値は既存サブコマンドと同様 `.cupola/cupola.toml` にする
  - _Requirements: 5.1_

- [x] 7.2 app.rs に cleanup コマンドのワイヤリングを追加する
  - `Command::Cleanup` が来た場合に設定を読み込み、DB と worktree アダプターを初期化して `CleanupUseCase::execute()` を呼び出す
  - 実行結果を標準出力に表示する
  - Task 5, 7.1 が完了していることが前提
  - _Requirements: 5.1, 5.5_

- [x] 8. テストを追加・更新する
- [x] 8.1 reset_for_restart のフィールド保持を検証するテストを追加する
  - in-memory SQLite を使用して `reset_for_restart` 後も `design_pr_number`、`impl_pr_number`、`worktree_path`、`feature_name` が保持されることを確認する
  - `state`、`retry_count`、`current_pid`、`error_message` はリセットされることを確認する
  - _Requirements: 1.1, 1.2_

- [x] 8.2 (P) find_by_state のテストを追加する
  - in-memory SQLite を使用して Cancelled 状態の Issue のみが返されることを確認する
  - 複数の状態が混在する場合も Cancelled のみ取得されることを確認する
  - _Requirements: 5.1_

- [x] 8.3 (P) TransitionUseCase の RetryExhausted で cleanup が呼ばれないことを検証するテストを追加する
  - モックアダプターで `worktree.remove` が呼ばれないことを確認する
  - `github.close_issue` は引き続き呼ばれることを確認する
  - `IssueClosed` イベントでは cleanup が引き続き呼ばれることも確認する
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 8.4 (P) initialize_issue 冪等化のテストを追加する
  - worktree が存在しない場合に `worktree.create` が呼ばれ `design_starting` メッセージが投稿されることを確認する
  - worktree が存在する場合に `worktree.create` が呼ばれず `resuming_design` メッセージが投稿されることを確認する
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [x] 8.5 (P) step7 の PR チェックテストを追加する
  - `design_pr_number` がある Issue で `PrStatus::Merged` を返すモックの場合、スポーンがスキップされ `DesignPrMerged` イベントが生成されることを確認する
  - `PrStatus::Open` の場合、`ProcessCompletedWithPr` イベントが生成されることを確認する
  - `PrStatus::Closed` の場合、通常通りスポーンされることを確認する
  - API エラーの場合もフェイルセーフで通常スポーンされることを確認する
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_

- [x] 8.6 (P) CleanupUseCase のテストを追加する
  - モックアダプターを使用して Cancelled Issue の worktree が削除されることを確認する
  - ブランチ（cupola/N/main, cupola/N/design）が削除されることを確認する
  - DB フィールドが NULL にリセットされることを確認する
  - worktree が存在しない場合はスキップされることを確認する
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6_

- [x] 8.7 (P) CLI の Cleanup サブコマンドパースをテストする
  - `cupola cleanup` のパースが正しく `Command::Cleanup` になることを確認する
  - `--config` オプションが正しく渡されることを確認する
  - _Requirements: 5.1_
