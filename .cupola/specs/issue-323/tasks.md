# 実装タスク一覧

- [ ] 1. 基盤となる context.rs と shared.rs を作成する
- [ ] 1.1 (P) ExecuteContext struct を context.rs に定義する
  - `src/application/polling/execute/context.rs` を新規作成する
  - generic param 6 個（G, I, P, C, W, F）と where 句を定義する
  - 9 フィールド（github, issue_repo, process_repo, claude_runner, worktree, file_gen, session_mgr, init_mgr, config）を宣言する
  - `use super::SpawnableGitWorktree` で W bound を参照する
  - _Requirements: 2.1_

- [ ] 1.2 (P) 共通ヘルパー関数を shared.rs に移動する
  - `src/application/polling/execute/shared.rs` を新規作成する
  - `get_pr_number_for_type`、`state_from_phase`、`phase_for_type`、`find_last_error` を移動する
  - 各関数を `pub(super)` に設定する（execute モジュール内からのみアクセス可能）
  - _Requirements: 1.1, 1.4_

- [ ] 2. 軽量な Effect executor を作成する（context.rs に依存）
- [ ] 2.1 (P) コメント系 Effect の実装を comment_executor.rs に移動する
  - `src/application/polling/execute/comment_executor.rs` を新規作成する
  - `post_completed`、`post_cancel`、`post_retry_exhausted`、`post_ci_fix_limit`、`reject_untrusted` を実装する
  - 各関数シグネチャを `ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>` ベースに統一する
  - 関数から `#[allow(clippy::too_many_arguments)]` を除去する
  - `post_retry_exhausted` は `shared::find_last_error` を呼び出す
  - _Requirements: 1.1, 1.2, 1.5, 2.3, 2.4_

- [ ] 2.2 (P) ワークツリー系 Effect の実装を worktree_executor.rs に移動する
  - `src/application/polling/execute/worktree_executor.rs` を新規作成する
  - `cleanup`、`switch_to_impl_branch` を `pub(super)` で実装する
  - `cleanup` は `super::retry_db_update` を呼び出す
  - _Requirements: 1.1, 1.2, 1.5, 2.3, 2.4_

- [ ] 2.3 (P) CloseIssue Effect の実装を close_executor.rs に移動する
  - `src/application/polling/execute/close_executor.rs` を新規作成する
  - `close` を `pub(super)` で実装する
  - `super::retry_db_update` を呼び出す
  - _Requirements: 1.1, 1.2, 1.5, 2.3, 2.4_

- [ ] 3. SpawnInit Effect の実装を spawn_init_executor.rs に移動する
  - `src/application/polling/execute/spawn_init_executor.rs` を新規作成する
  - `execute`（`pub(super)`）、`spawn_init_task`、`prepare_init_handle`、`perform_init_sync` を移動する
  - `execute` を `spawn_init_task` を呼び出すエントリ関数として実装する
  - `perform_init_sync` の `#[allow(clippy::too_many_arguments)]` を除去する
  - 既存テスト（`perform_init_sync_*`、`prepare_init_handle_*`）を `#[cfg(test)]` ブロックに移動する
  - テストで使用するモック型（`MockGitWorktree`、`MockFileGenerator`、`MockGitHubForInit`、`MockProcRepo`、ヘルパー関数）も本ファイルの `#[cfg(test)]` に移動する
  - _Requirements: 1.1, 1.2, 1.5, 2.3, 2.4, 5.3_

- [ ] 4. SpawnProcess Effect の実装を spawn_process_executor.rs に移動する
  - `src/application/polling/execute/spawn_process_executor.rs` を新規作成する
  - `execute`（`pub(super)`）、`spawn_process`、`prepare_process_spawn`、`handle_body_tampered` を移動する
  - `BodyTamperedError` は `use super::BodyTamperedError` で、`sha256_hex` は `use super::sha256_hex` で参照する
  - `spawn_process` と `prepare_process_spawn` の `#[allow(clippy::too_many_arguments)]` を除去する
  - `prepare_process_spawn` をテストから参照できるよう `pub(super)` に設定する
  - `shared::get_pr_number_for_type`、`shared::state_from_phase`、`shared::phase_for_type` を呼び出す
  - 既存の `prepare_process_spawn` 系テストを `#[cfg(test)]` ブロックに移動する
  - _Requirements: 1.1, 1.2, 1.5, 2.3, 2.4, 5.3_

- [ ] 5. dispatcher.rs を作成し execute_one を置き換える
  - `src/application/polling/execute/dispatcher.rs` を新規作成する
  - `dispatch` 関数を `pub(super)` で実装する（exhaustive match）
  - 全 10 個の Effect バリアント（`PostCompletedComment`、`PostCancelComment`、`PostRetryExhaustedComment`、`RejectUntrustedReadyIssue`、`PostCiFixLimitComment`、`SpawnInit`、`SpawnProcess`、`SwitchToImplBranch`、`CleanupWorktree`、`CloseIssue`）を対応する executor 関数に委譲する
  - `SpawnProcess` の payload（type_, causes, pending_run_id）をアンパックして渡す
  - `#[allow(clippy::too_many_arguments)]` は不要（ctx 1 引数で全リソースアクセス）
  - _Requirements: 1.1, 3.1, 3.2, 3.3_

- [ ] 6. mod.rs を整備して execute/ ディレクトリを完成させる
  - `src/application/polling/execute.rs` を `src/application/polling/execute/mod.rs` に変換する（ディレクトリ作成と移動）
  - 全サブモジュール（`mod context`、`mod dispatcher`、`mod comment_executor`、`mod spawn_init_executor`、`mod spawn_process_executor`、`mod worktree_executor`、`mod close_executor`、`mod shared`）を宣言する
  - `execute_effects` の内部実装を `execute_one` 呼び出しから `ExecuteContext` 構築 + `dispatcher::dispatch` 呼び出しに置き換える
  - `execute_one` 関数をファイルから削除する
  - `spawn_init_task`、`prepare_init_handle`、`perform_init_sync`、`spawn_process`、`prepare_process_spawn`、`handle_body_tampered`、共通ヘルパー 4 関数をファイルから削除する（各 executor・shared.rs に移動済み）
  - `execute_effects` の `#[allow(clippy::too_many_arguments)]` を除去する
  - テスト（`sha256_hex_*`、`body_tampered_*`、`effect_priority_order`、`best_effort_classification`、`fixing_causes_*`）を mod.rs の `#[cfg(test)]` ブロックに維持する
  - _Requirements: 1.1, 1.3, 1.5, 2.2, 4.1, 4.2, 4.3, 5.4_

- [ ] 7. コード品質を検証する
- [ ] 7.1 (P) 静的解析とフォーマットを通過させる
  - `devbox run clippy` を実行して全警告を解消する（`-D warnings` 相当）
  - `devbox run fmt-check` を実行してフォーマット差分を解消する
  - 残存する `#[allow(clippy::too_many_arguments)]` がゼロであることを確認する
  - _Requirements: 5.1, 5.2, 2.4_

- [ ] 7.2 (P) 全テストの通過を確認する
  - `devbox run test` を実行して全テストが通過することを確認する
  - 既存の統合テストが `execute_effects` シグネチャ変更なしで通過することを確認する
  - 各ファイルが 500 行以下であることをファイル行数で確認する
  - _Requirements: 5.3, 5.4, 1.2_
