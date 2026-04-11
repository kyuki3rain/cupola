# Requirements Document

## Introduction

`src/application/polling/resolve.rs` の Running フェーズ（`DesignRunning` / `ImplementationRunning`）では、GitHub API 呼び出し（`find_pr_by_branches` / `create_pr`）に `?` 演算子を使用しており、1 件のセッションで PR 作成が失敗するとエラーがそのまま伝播して `process_exited_session` ループ全体が中断される。これにより同一ポーリングサイクル内の他セッションの `ProcessRun` 更新が行われず、状態機械が停滞または誤再実行を引き起こす可能性がある。

本機能は、GitHub API エラーを当該セッションの `ProcessRun` 失敗として記録し、`Ok(())` を返してポーリングループを継続させることで、兄弟セッションへの影響を排除する。

## Requirements

### Requirement 1: Running フェーズ GitHub API エラーの局所化

**Objective:** ポーリングシステムとして、Running フェーズの GitHub API 失敗を当該セッション単位に閉じ込めたい。そうすることで、同一サイクル内の他セッションの処理が妨げられないようにする。

#### Acceptance Criteria

1. When `find_pr_by_branches()` returns an error during the Running phase, the Resolve module shall call `process_repo.mark_failed(run_id, Some(error_message))` with the error string and return `Ok(())`.
2. When `create_pr()` returns an error during the Running phase, the Resolve module shall call `process_repo.mark_failed(run_id, Some(error_message))` with the error string and return `Ok(())`.
3. If `run_id` is 0 at the time of GitHub API failure, the Resolve module shall skip the `mark_failed` call and return `Ok(())`.
4. When a GitHub API error occurs during the Running phase, the Resolve module shall emit a `tracing::warn!` log containing `run_id`, `head_branch`, `base_branch`, and the error detail.
5. The Resolve module shall propagate errors from `process_repo.mark_failed()` itself using `?`, as database layer failures are unrecoverable for the loop.

### Requirement 2: ポーリングループ継続性の保証

**Objective:** ポーリングシステムとして、Running フェーズで 1 セッションの GitHub API 失敗が発生しても後続セッションの処理が継続されることを保証したい。そうすることで、状態機械の停滞や誤再実行を防ぐ。

#### Acceptance Criteria

1. When a GitHub API error is handled locally in the Running phase, the Resolve module shall return `Ok(())` so the caller loop continues to the next session.
2. While two or more sessions complete in the same polling cycle and one session's PR creation fails, the Resolve module shall process the remaining sessions' `ProcessRun` updates in the same cycle.
3. The Resolve module shall not propagate GitHub API errors from `find_pr_by_branches()` or `create_pr()` to the caller.

### Requirement 3: 失敗セッションの ProcessRun 状態整合性

**Objective:** ポーリングシステムとして、GitHub API 失敗時に該当セッションの `ProcessRun` を正しく `failed` 状態に遷移させたい。そうすることで、ポーリングループ上流で状態機械が正確な情報に基づいてリトライや遷移を決定できるようにする。

#### Acceptance Criteria

1. When the Running phase GitHub API call fails, the Resolve module shall set `ProcessRun.state = failed`, populate `error_message` with the error string, clear `pid`, and set `finished_at` via `mark_failed()`.
2. The Resolve module shall not call `mark_succeeded()` for a session whose PR creation failed due to a GitHub API error.

### Requirement 4: テスト検証

**Objective:** 開発者として、修正が正しく動作することをユニットテストおよびインテグレーションテストで検証したい。そうすることで、将来のリグレッションを防ぐ。

#### Acceptance Criteria

1. When `find_pr_by_branches()` is stubbed to return an error, the unit test shall verify that `mark_failed()` is called with the correct `run_id` and error message, and the function returns `Ok(())`.
2. When `create_pr()` is stubbed to return an error, the unit test shall verify that `mark_failed()` is called with the correct `run_id` and error message, and the function returns `Ok(())`.
3. When two concurrent sessions complete in the same cycle and one session's GitHub API call is stubbed to fail, the integration test shall verify that the failed session has `ProcessRun.state = failed` with `error_message` populated, and the other session's `ProcessRun` is updated normally.
4. The Resolve module shall emit `tracing::warn!` logs containing `run_id`, `head_branch`, `base_branch`, and the error detail when a GitHub API error is handled locally, verifiable via test log capture.
