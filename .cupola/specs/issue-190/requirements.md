# Requirements Document

## Project Description (Input)
## Summary

The polling `CleanupWorktree` effect (src/application/polling/execute.rs ~lines 211-228) removes the worktree directory but DOES NOT delete the `cupola/{issue}/main` and `cupola/{issue}/design` branches. However, the CLI cleanup command (`src/application/cleanup_use_case.rs` ~lines 81-94) DOES delete both branches. This creates an asymmetry in cleanup behavior between polling-triggered completion and explicit CLI cleanup.

## Current Behavior Comparison

| Behavior | Polling CleanupWorktree | CLI cleanup |
|----------|------------------------|------------|
| Removes worktree directory | ✓ Yes | ✓ Yes |
| Deletes `cupola/{n}/main` branch | ✗ No | ✓ Yes (both local & remote) |
| Deletes `cupola/{n}/design` branch | ✗ No | ✓ Yes (both local & remote) |
| Clears `worktree_path` metadata | ✓ Yes | ✓ Yes (if worktree removed) |

## Design Spec Evidence

From `docs/architecture/effects.md` line 11 (Effects section):

> `Completed` では CleanupWorktree を持続性エフェクトとして実行し、`worktree_path` がクリアされるまで毎サイクル再試行する。`Cancelled` では polling loop は worktree・branch を保持し、再起動時に再利用できるようにする（`cupola cleanup` コマンドを実行した場合は明示的に削除される）。

This states:
1. **Completed**: CleanupWorktree is persistent and retried until `worktree_path` is cleared
2. **Cancelled**: Polling preserves worktree & branch for reuse on restart; only explicit `cupola cleanup` deletes them

The spec does NOT mention branch deletion as part of CleanupWorktree, but `Cancelled` preserves branches for CLI cleanup to handle explicitly.

## Key Question: PR Lifecycle & Branch Deletion Risk

After merging an implementation PR:

1. **Remote branch status**: GitHub may auto-delete merged PR branches (depends on repository settings), so the remote `cupola/{n}/main` / `cupola/{n}/design` may already be gone at Completed time
2. **Local branch safety**: `delete_branch()` in `git_worktree_manager.rs` is idempotent (lines 135-141):
   - Local delete: `git branch -D` (fails silently if missing)
   - Remote delete: `git push origin --delete` (fails silently if missing)
   - Always returns `Ok(())` regardless

3. **No risk from deletion**: Even if the branch doesn't exist, the operation succeeds (best-effort pattern)

## Proposed Fix

The polling `CleanupWorktree` effect should also delete both branches on success, matching CLI behavior. This:
- **Reduces dangling references**: Prevents accumulation of merged branch refs after Completed
- **Matches user expectations**: Completing an issue fully cleans up all its artifacts
- **Consistent with CLI**: Both paths delete the same artifacts
- **Safe**: `delete_branch()` is idempotent, so missing branches won't cause failures
- **Supports desired model**: Completed = full cleanup; only Cancelled preserves for manual restart

## Risk Assessment

**Low risk**:
- Idempotent operation (can be safely re-executed)
- PR is already merged, no active development on those branches
- Remote may already have deleted the branch
- No change to worktree removal logic (just adds branch cleanup)

**Potential caveat**: If external tools reference these local branches before cleanup completes, they would fail to find them. However:
- Branches are managed by cupola exclusively
- Completed state means no further work on this issue
- Impact is acceptable

## Test Plan

- [ ] Unit test: Verify `CleanupWorktree` calls `worktree.delete_branch()` for both `cupola/{n}/main` and `cupola/{n}/design`
- [ ] Unit test: Verify both branches are deleted even if worktree removal fails (best-effort)
- [ ] Integration test: Polling completion on a merged PR also removes local branches
- [ ] Regression test: Ensure `Cancelled` still preserves branches (no change to Cancelled behavior)
- [ ] E2E test: Full issue lifecycle including Completed branch cleanup

## Implementation Notes

Modify `src/application/polling/execute.rs` `Effect::CleanupWorktree` block (lines 211-228):
1. After worktree removal, also attempt to delete both branches
2. Use the idempotent `worktree.delete_branch()` method
3. Log branch deletion results
4. Update metadata only after both worktree & branch cleanup attempts

---

**Discovered during**: Code review of cleanup patterns across polling vs CLI paths

## Requirements

### Requirement 1: ポーリング完了時のブランチ削除

**Objective:** Cupola 運用者として、issue が Completed になったときにワークツリーと同時にブランチも削除されることを望む。そうすることで、マージ済みブランチの残留を防ぎ、ローカルリポジトリをクリーンな状態に保てる。

#### Acceptance Criteria

1.1. When `Effect::CleanupWorktree` が実行されワークツリーパスが存在する場合, the polling executor shall `cupola/{n}/main` および `cupola/{n}/design` ブランチの削除を試みる

1.2. When `delete_branch` が成功した場合, the polling executor shall 削除されたブランチ名とイシュー番号をログに記録する

1.3. If ブランチが存在しない（既にリモートで削除済み等）, the polling executor shall エラーを発生させず正常に処理を続行する（冪等性）

1.4. The polling executor shall ブランチ削除の結果にかかわらず `worktree_path` メタデータのクリアを継続して実行する

1.5. When ワークツリーパスが存在しない場合, the polling executor shall ブランチ削除をスキップして `worktree_path` のクリアに進む

### Requirement 2: Cancelled 状態での非破壊的保持

**Objective:** Cupola 運用者として、issue が Cancelled になった場合にワークツリーとブランチが保持されることを望む。そうすることで、再起動時に作業を再開できる。

#### Acceptance Criteria

2.1. While issue の状態が Cancelled である間, the polling loop shall ワークツリーおよびブランチを削除しない

2.2. The polling executor shall Cancelled 状態の issue に対して `Effect::CleanupWorktree` を適用しない（既存の動作を維持する）

### Requirement 3: テスト網羅性

**Objective:** 開発者として、CleanupWorktree エフェクトのブランチ削除動作が自動テストで検証されることを望む。そうすることで、将来の変更による回帰を防げる。

#### Acceptance Criteria

3.1. The test suite shall `CleanupWorktree` エフェクト実行時に `delete_branch` が `cupola/{n}/main` と `cupola/{n}/design` の両方に対して呼び出されることを検証する

3.2. The test suite shall `delete_branch` がエラーを返してもエフェクト全体が失敗しないことを検証する

3.3. The test suite shall Cancelled 状態の issue に対してブランチ削除が実行されないことをリグレッションテストで検証する
