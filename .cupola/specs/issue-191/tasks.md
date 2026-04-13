# Implementation Plan

- [x] 1. prepare_process_spawn の ProcessRun 作成順序を変更する
- [x] 1.1 ProcessRun 保存をマージ処理の前に移動する
  - `ProcessRun::new_running()` および `process_repo.save()` の呼び出しを、マージ処理ブロックより前に移動する
  - 既存の `run_id` 参照箇所がすべて新しい順序で正しく参照されることを確認する
  - _Requirements: 2.2_

- [x] 2. マージエラーの種別判定ロジックを実装する
- [x] 2.1 worktree.merge の結果を match でパターンマッチし has_merge_conflict を設定する
  - `Ok(())` の場合は `has_merge_conflict = false`
  - `Err(e) if e.is::<MergeConflictError>()` の場合は `has_merge_conflict = true` かつ `tracing::info!` でログ記録
  - その他の `Err(e)` の場合は `tracing::error!` でログ記録し `mark_failed` を呼んで `return Err(e)` する
  - コンフリクトが意図的であることをコードコメントで明示する
  - 致命的エラーが非期待であることをコードコメントで明示する
  - `mark_failed` 呼び出しは `let _ = ...` の best-effort パターンで実施する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.3_

- [x] 2.2 has_merge_conflict フラグを build_session_config に渡す
  - 既存のハードコード `false` を 2.1 で設定した `has_merge_conflict` 変数に置き換える
  - TODO コメントを削除する
  - _Requirements: 3.1, 3.2, 3.3_

- [x] 3. ユニットテストを追加する
- [x] 3.1 (P) MergeConflictError 時にスポーン継続・mark_failed 不呼び出しを検証するテストを書く
  - `SpawnableGitWorktree` モックが `MergeConflictError` を返すケースを設定する
  - `mark_failed` が呼ばれていないことを検証する
  - スポーンが実行されることを検証する
  - _Requirements: 4.1_

- [x] 3.2 (P) 非コンフリクトエラー時にスポーン中断・mark_failed 呼び出しを検証するテストを書く
  - `SpawnableGitWorktree` モックがネットワークエラー相当の `anyhow::Error` を返すケースを設定する
  - `mark_failed` が呼ばれたことを検証する
  - スポーンが実行されないことを検証する
  - _Requirements: 4.2_
