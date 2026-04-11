# Implementation Plan

- [ ] 1. Running フェーズの GitHub API エラーハンドリングを実装する
- [ ] 1.1 `find_pr_by_branches` と `create_pr` のエラーを局所化し `mark_failed` を呼び出す
  - `resolve.rs` 行 289–309 の Running フェーズブロックを修正する
  - `find_pr_by_branches().await?` の `?` を除去し `if let Err(e)` パターンに置き換える
  - `create_pr().await?` の `?` を除去し `if let Err(e)` パターンに置き換える
  - エラー発生時は `run_id > 0` を確認してから `mark_failed(run_id, Some(e.to_string())).await?` を呼び出す
  - その後 `return Ok(())` して後続コードに到達しないようにする
  - `mark_failed` 自体のエラーは `?` で伝播させる
  - _Requirements: 1.1, 1.2, 1.3, 1.5, 2.1, 2.3, 3.1, 3.2_

- [ ] 1.2 エラー発生時に `tracing::warn!` ログを追加する
  - `run_id`・`head_branch`・`base_branch`・エラー詳細（`%e`）を含む `tracing::warn!` を発行する
  - Fixing フェーズ（行 316–326）の既存ログスタイルに合わせる
  - _Requirements: 1.4_

- [ ] 2. ユニットテストを実装する
- [ ] 2.1 (P) `find_pr_by_branches` 失敗時のユニットテストを追加する
  - `find_pr_by_branches` がエラーを返すモック `GitHubClient` を用意する
  - `process_exited_session` を呼び出し `mark_failed` が正しい引数で呼ばれたことを確認する
  - 関数が `Ok(())` を返すことを確認する
  - `mark_succeeded` が呼ばれていないことを確認する
  - _Requirements: 1.1, 2.1, 3.1, 3.2, 4.1_

- [ ] 2.2 (P) `create_pr` 失敗時のユニットテストを追加する
  - `find_pr_by_branches` が `None` を返し `create_pr` がエラーを返すモックを用意する
  - `process_exited_session` を呼び出し `mark_failed` が正しい引数で呼ばれたことを確認する
  - 関数が `Ok(())` を返すことを確認する
  - `mark_succeeded` が呼ばれていないことを確認する
  - _Requirements: 1.2, 2.1, 3.1, 3.2, 4.2_

- [ ] 2.3 (P) `run_id == 0` かつ GitHub API エラー時のユニットテストを追加する
  - `run_id = 0` でセッションを構成し `find_pr_by_branches` がエラーを返すモックを用意する
  - `mark_failed` が呼ばれないことを確認する
  - 関数が `Ok(())` を返すことを確認する
  - _Requirements: 1.3, 2.1_

- [ ] 3. インテグレーションテストを実装する
- [ ] 3.1 2 セッション並行処理のインテグレーションテストを追加する
  - セッション A の GitHub API 呼び出しをスタブでエラーにし、セッション B は正常に動作するよう設定する
  - 同一ポーリングサイクルで両セッションを処理し、セッション A の `ProcessRun.state = failed / error_message 設定済み`、セッション B の `ProcessRun` が正常更新されることを確認する
  - _Requirements: 2.2, 4.3_

- [ ] 3.2 (P) エラーログ出力の検証テストを追加する
  - `tracing_test` などのログキャプチャ手段を用いて GitHub API エラー発生時のログを確認する
  - `run_id`・`head_branch`・`base_branch`・エラー詳細が含まれることを確認する
  - _Requirements: 1.4, 4.4_
