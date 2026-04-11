# Requirements Document

## Introduction

Fixing フェーズ（DesignFix / ImplFix）のスポーン前に行われる `worktree.merge()` 呼び出しにおいて、エラーの種別が区別されていない問題を修正する。現在はすべてのマージエラーが `warn` ログのみで握りつぶされ、Claude Code セッションがスポーンされてしまう。マージコンフリクト（exit code 1）は Claude が解決すべき意図的な状態であるが、ネットワーク障害・権限エラーなどの致命的エラー（exit code 2+）は監査証跡を残してスポーンを中断すべきである。

## Requirements

### Requirement 1: マージエラーの種別判定と処理分岐

**Objective:** オペレーターとして、マージエラーの種別（コンフリクト vs. 致命的エラー）に応じた適切な処理が行われることを望む。これにより、Claude が解決すべき状況と運用者が介入すべき状況を明確に区別できる。

#### Acceptance Criteria

1. When `worktree.merge()` が `MergeConflictError` を返した場合, the スポーンシステム shall コンフリクトが検出されたことを `info` レベルでログに記録し、スポーンを継続する。
2. When `worktree.merge()` が `MergeConflictError` 以外のエラーを返した場合, the スポーンシステム shall エラーを `error` レベルでログに記録し、スポーンを中断して `Err` を返す。
3. The スポーンシステム shall `MergeConflictError` がローカルコンフリクトであり Claude Code による解決を意図していることをコードコメントで明示する。
4. The スポーンシステム shall その他のマージエラーが致命的エラーであることをコードコメントで明示する。

### Requirement 2: 非コンフリクトマージエラーの監査記録

**Objective:** オペレーターとして、致命的なマージエラーが `ProcessRun.error_message` に記録されることを望む。これにより、障害発生時の原因調査を可能にする。

#### Acceptance Criteria

1. When `worktree.merge()` が致命的エラーを返した場合, the スポーンシステム shall `process_run_repository.mark_failed(run_id, Some(error_message))` を呼び出してエラーメッセージを記録する。
2. When 致命的マージエラーが記録される場合, the スポーンシステム shall `ProcessRun` をスポーン前に作成して `run_id` を確保する。
3. If `mark_failed` の呼び出しに失敗した場合, the スポーンシステム shall そのエラーを無視し（best-effort）、マージエラー自体を `Err` として返す。

### Requirement 3: マージコンフリクトフラグの設定

**Objective:** 開発者として、`MergeConflictError` が検出された場合に `has_merge_conflict` フラグが `true` に設定されることを望む。これにより、Claude Code セッションがコンフリクト解決を自動的に試みられるようになる。

#### Acceptance Criteria

1. When `worktree.merge()` が `MergeConflictError` を返した場合, the スポーンシステム shall `build_session_config` に `has_merge_conflict = true` を渡す。
2. When `worktree.merge()` が成功した場合（`Ok(())`）, the スポーンシステム shall `build_session_config` に `has_merge_conflict = false` を渡す。
3. The スポーンシステム shall `has_merge_conflict` フラグを `MergeConflictError` のパターンマッチから直接導出する（`ProcessRun` フィールドへの保存は不要）。

### Requirement 4: テスト

**Objective:** 開発者として、マージエラーの種別判定ロジックがユニットテストで検証されることを望む。

#### Acceptance Criteria

1. When `worktree.merge()` が `MergeConflictError` を返すモックを用いた場合, the テスト shall `mark_failed` が呼ばれないこと、および `has_merge_conflict = true` でスポーンが実行されることを検証する。
2. When `worktree.merge()` がネットワークエラーなどの非コンフリクトエラーを返すモックを用いた場合, the テスト shall `mark_failed` が呼ばれること、およびスポーンが実行されないことを検証する。
