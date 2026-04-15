# Requirements Document

## Introduction

Fixing spawn（`DesignFix` / `ImplFix`）フェーズで `worktree.merge()` を呼び出す直前に `worktree.fetch()` が実行されていないため、ローカルの `origin/main` ref が古いまま merge が行われ、GitHub 上で再び `CONFLICTING` 状態が発生するバグを修正する。

## Requirements

### Requirement 1: Fixing spawn 前の fetch 実行

**Objective:** As a Cupola ユーザー, I want Fixing spawn 時に必ず最新の `origin/main` を取得してから merge してほしい, so that 古い ref に基づく merge が原因で発生する `mergeable=CONFLICTING` の再発を防げる.

#### Acceptance Criteria

1. When `ProcessRunType::DesignFix` または `ProcessRunType::ImplFix` の spawn が開始されるとき, the PollingUseCase shall `worktree.merge()` を呼び出す前に `worktree.fetch()` を実行する.
2. If `worktree.fetch()` がエラーを返したとき, the PollingUseCase shall 警告ログを出力し、fetch 失敗を fatal とせずに後続の merge 処理を継続する.
3. The PollingUseCase shall fetch 成功・失敗に関わらず `worktree.merge()` を呼び出し、既存の merge エラー処理（`MergeConflictError` 分岐と非 conflict warn）を維持する.

### Requirement 2: テストによる動作保証

**Objective:** As a 開発者, I want fetch → merge の順序と fetch 失敗時の継続動作がテストで保証されてほしい, so that リグレッションを早期に検出できる.

#### Acceptance Criteria

1. When fetch が成功するとき, the unit test shall `fetch()` が `merge()` より先に呼ばれたことを検証する.
2. When fetch がエラーを返すとき, the unit test shall 処理が中断されず `merge()` が呼ばれることを検証する.
3. The unit test shall 既存の `DesignFix` / `ImplFix` 以外の spawn タイプで fetch が呼ばれないことを検証する.
