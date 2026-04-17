# 実装計画

- [ ] 1. `retry_db_update` ヘルパー関数を実装する
- [ ] 1.1 バックオフ定数とヘルパー関数のシグネチャを定義する
  - `RETRY_DELAYS: [Duration; 3] = [1s, 2s, 4s]` をコンパイル時定数として定義する
  - `retry_db_update<F, Fut>(issue_number, effect_name, db_update)` の非同期関数を `execute.rs` 内プライベート関数として追加する
  - 関数のジェネリック境界（`Fn() -> Fut`, `Future<Output = Result<()>>`）を型安全に定義する
  - _Requirements: 1.4, 1.5_

- [ ] 1.2 リトライループとバックオフロジックを実装する
  - バックオフ配列をイテレートし、各試行で `db_update()` を呼び出す
  - 成功時は即座に `Ok(())` を返す
  - 失敗時は `tokio::time::sleep(delay)` で非同期待機してから次の試行へ進む
  - _Requirements: 1.1, 1.2, 1.3, 1.5_

- [ ] 1.3 構造化ログ出力を実装する
  - 各リトライ失敗時に `tracing::warn!(issue_number, effect, attempt, error = %e, "DB update failed, retrying")` を出力する
  - 全リトライ消費後に `tracing::error!(issue_number, effect, max_attempts, error = %e, "DB update permanently failed after API success")` を出力する
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 2. `CloseIssue` ハンドラを `retry_db_update` を使用するよう修正する
  - `execute.rs` の `Effect::CloseIssue` ブランチで、`issue_repo.update_state_and_metadata(...).await?` の呼び出しを `retry_db_update` 経由に変更する
  - `issue.close_finished = true` のインメモリ更新は `retry_db_update` 成功後にのみ実行されることを確認する（既存の制御フローを維持）
  - _Requirements: 1.1_

- [ ] 3. `CleanupWorktree` ハンドラを `retry_db_update` を使用するよう修正する
  - `execute.rs` の `Effect::CleanupWorktree` ブランチで、`worktree_path` クリアの `update_state_and_metadata` 呼び出しを `retry_db_update` 経由に変更する
  - `issue.worktree_path = None` のインメモリ更新は `retry_db_update` 成功後にのみ実行されることを確認する
  - _Requirements: 1.2_

- [ ] 4. `retry_db_update` ヘルパーのユニットテストを追加する
- [ ] 4.1 (P) リトライ成功ケースのテストを実装する
  - クロージャが 1 回失敗後に成功するモックを用意する
  - `retry_db_update` が `Ok(())` を返すことを検証する
  - クロージャが 2 回呼ばれたことを検証する
  - _Requirements: 3.1, 3.3_

- [ ] 4.2 (P) 全リトライ失敗ケースのテストを実装する
  - クロージャが常に失敗するモックを用意する
  - `retry_db_update` が `Err` を返すことを検証する
  - クロージャが 3 回呼ばれたことを検証する
  - _Requirements: 3.3_

- [ ] 5. エフェクトハンドラのインテグレーションテストを追加する
- [ ] 5.1 (P) `CloseIssue` 部分失敗シナリオのテストを実装する
  - `MockIssueRepository` の `update_state_and_metadata` が 1 回失敗後に成功するよう設定する
  - `CloseIssue` エフェクト実行後に `issue.close_finished == true` になることを検証する
  - _Requirements: 3.1_

- [ ] 5.2 (P) `CloseIssue` 永続失敗時のインメモリ状態不変を検証するテストを実装する
  - `MockIssueRepository` の `update_state_and_metadata` が常に失敗するよう設定する
  - `CloseIssue` エフェクト実行後も `issue.close_finished == false` のままであることを検証する
  - _Requirements: 3.4_

- [ ] 5.3 (P) `CleanupWorktree` 永続失敗時のインメモリ状態不変を検証するテストを実装する
  - `MockIssueRepository` の `update_state_and_metadata` が常に失敗するよう設定する
  - `CleanupWorktree` エフェクト実行後も `issue.worktree_path` が変更されないことを検証する
  - _Requirements: 3.2, 3.4_
