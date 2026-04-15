# Implementation Plan

- [x] 1. Fixing spawn に fetch 呼び出しを追加する
  - `execute.rs` の `DesignFix` / `ImplFix` 分岐内、`worktree.merge()` の直前に `worktree.fetch()` の呼び出しを追加する
  - fetch 失敗時は `tracing::warn!(error = %e, "fetch failed before fixing merge, proceeding with stale origin")` を出力し、merge ステップへ継続する
  - merge 以降の既存ロジック（`MergeConflictError` 分岐、`write_review_threads_input`）は変更しない
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 2. テストを追加・更新する
- [x] 2.1 fetch → merge の順序と fetch 失敗時の継続をユニットテストで検証する
  - `execute.rs` 内の `#[cfg(test)] mod tests` に、`MockGitWorktree` を使ったテストケースを追加する
  - `DesignFix` / `ImplFix` タイプで fetch が merge より先に呼ばれることを検証する（要件 2.1）
  - fetch が `Err` を返しても merge が呼ばれることを検証する（要件 2.2）
  - `MockGitWorktree` の `fetch` フィールドに呼び出し記録機構を追加する（既存モックを修正）
  - _Requirements: 2.1, 2.2_

- [x] 2.2 非 Fixing タイプで fetch が呼ばれないことを検証する
  - `SpawnInit` 等の非 `DesignFix`/`ImplFix` タイプで fetch が呼ばれないことをテストする（要件 2.3）
  - 既存テストの `MockGitWorktree` 期待値が fetch 呼び出しを含まない場合はそのまま利用する
  - _Requirements: 2.3_
