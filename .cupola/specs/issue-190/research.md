# Research & Design Decisions

---
**Purpose**: 発見フェーズの調査結果、アーキテクチャ調査、技術設計の根拠を記録する。

---

## Summary

- **Feature**: `issue-190` — ポーリング完了時のブランチクリーンアップ非対称性の修正
- **Discovery Scope**: Simple Addition（既存システムへのベストエフォート操作の追加）
- **Key Findings**:
  - `Effect::CleanupWorktree` はワークツリー削除のみ実行し、ブランチ削除を行わない
  - `CleanupUseCase`（CLI cleanup）はワークツリーとブランチの両方を削除している
  - `delete_branch()` は冪等操作であり、ブランチが存在しない場合も `Ok(())` を返す
  - `Cancelled` 状態はブランチを保持する設計が意図的であり、この変更の対象外

## Research Log

### CleanupWorktree エフェクトの現在の実装

- **Context**: ポーリングループで Completed イシューに対して実行されるエフェクトのコードを調査
- **Sources Consulted**: `src/application/polling/execute.rs` lines 219–236
- **Findings**:
  - `Effect::CleanupWorktree` は `issue.worktree_path` が存在する場合にワークツリーを削除する
  - `worktree_path` を DB でクリアする
  - ブランチ削除の処理は一切含まれていない
- **Implications**: この実装でポーリング完了後にブランチが残留する

### CLI CleanupUseCase の実装

- **Context**: 非対称性を確認するため CLI クリーンアップのコードを調査
- **Sources Consulted**: `src/application/cleanup_use_case.rs` lines 70–94
- **Findings**:
  - ワークツリー削除後に `cupola/{n}/main` と `cupola/{n}/design` の両ブランチを削除する
  - ブランチ削除はベストエフォートで、失敗時は `warn` ログを出して継続する
  - `worktree_removed` フラグで DB クリーンアップの条件を制御している
- **Implications**: ポーリング側も同じパターンを踏襲する必要がある

### delete_branch の冪等性

- **Context**: ブランチが存在しない場合の安全性を確認
- **Sources Consulted**: `src/adapter/outbound/git_worktree_manager.rs` lines 155–170、ユニットテスト `delete_branch_is_always_ok`
- **Findings**:
  - ローカル: `git branch -D` — ブランチ不在でも `Ok(())` を返す（失敗を無視）
  - リモート: `git push origin --delete` — リモートブランチ不在でも `Ok(())` を返す（失敗を無視）
  - 既存のテスト `t_4_wt_3_delete_branch_idempotent_on_missing` が冪等性を検証済み
- **Implications**: 不存在ブランチへの削除呼び出しは安全。実装にガード不要

### GitWorktree トレイト定義

- **Context**: アプリケーション層からの利用インターフェースを確認
- **Sources Consulted**: `src/application/port/git_worktree.rs`
- **Findings**:
  - `fn delete_branch(&self, branch: &str) -> Result<()>` がトレイトに定義済み
  - `polling/execute.rs` 内のモック `MockGitWorktree` にも実装済み（`fn delete_branch(&self, _b: &str) -> Result<()>`）
- **Implications**: 新しいトレイトメソッドやモック変更は不要。既存インターフェースを使用

### Cancelled 状態の設計意図

- **Context**: `docs/architecture/effects.md` の設計記述を確認
- **Sources Consulted**: `docs/architecture/effects.md` line 11
- **Findings**:
  - `Completed`: CleanupWorktree を持続性エフェクトとして実行し `worktree_path` がクリアされるまで再試行
  - `Cancelled`: polling loop はワークツリー・ブランチを保持し、再起動時に再利用できるようにする
- **Implications**: Cancelled 状態のブランチ保持は意図的な設計であり、今回の変更対象外

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| CleanupWorktree にブランチ削除を追加 | 既存 CleanupWorktree ブロックにブランチ削除呼び出しを直接追加 | シンプル、CLI と同一パターン、影響範囲最小 | なし（冪等操作のため） | 採用 |
| 新規エフェクト `CleanupBranches` の追加 | ブランチ専用エフェクトを定義して別途スケジュール | 関心の分離 | 不要な複雑性、今回の要件には過剰 | 不採用 |

## Design Decisions

### Decision: ワークツリー削除後にブランチ削除を実行する

- **Context**: ブランチ削除のタイミングをワークツリー削除の前後どちらにするかの選択
- **Alternatives Considered**:
  1. ワークツリー削除前にブランチ削除 — ブランチが消えた後にワークツリー操作が失敗した場合の復旧が複雑になる
  2. ワークツリー削除後にブランチ削除（CLI と同じ順序）— CLI の実装と整合性がある
- **Selected Approach**: ワークツリー削除後にブランチ削除を実行する
- **Rationale**: CLI の `CleanupUseCase` と同じ実行順序を踏襲することで一貫性を保つ。ブランチ削除は冪等なので再試行でも安全
- **Trade-offs**: ワークツリー削除に失敗しても CleanupWorktree は次サイクルで再試行されるため、その場合はブランチ削除も再度試みられる（問題なし）
- **Follow-up**: なし（冪等性により再試行は安全）

### Decision: ブランチ削除はベストエフォートとして扱う

- **Context**: ブランチ削除の失敗がエフェクト全体の失敗に影響すべきか
- **Alternatives Considered**:
  1. 失敗時にエラーを伝播する — `worktree_path` クリアが実行されず、次サイクルで再試行される
  2. 失敗時に `warn` ログを出して継続する（CLI と同じパターン）— `worktree_path` クリアは確実に実行される
- **Selected Approach**: 失敗時に `warn` ログを出して継続する
- **Rationale**: CLI の実装と一致するパターン。ブランチ削除の失敗（例: 既に削除済み）は運用上の問題ではなく、クリーンアップの主目的（`worktree_path` クリア）を妨げるべきでない
- **Trade-offs**: ブランチが残留する可能性が理論上あるが、`delete_branch` は既に冪等実装であり現実的なリスクは低い
- **Follow-up**: なし

## Risks & Mitigations

- ブランチが既に削除されている場合のエラー — `delete_branch` が冪等実装のため発生しない
- Cancelled イシューへの誤適用 — `CleanupWorktree` エフェクト自体が Completed のみに適用されるため、Cancelled では発火しない（既存の状態機械ロジックによる保護）
- 将来的なブランチ命名規則の変更 — ブランチ名の生成ロジックが変わった場合はこのコードも合わせて変更が必要（現状は低リスク）

## References

- `src/application/polling/execute.rs` — CleanupWorktree エフェクトの現在の実装
- `src/application/cleanup_use_case.rs` — CLI cleanup のブランチ削除実装（参照実装）
- `src/adapter/outbound/git_worktree_manager.rs` — delete_branch の冪等実装
- `src/application/port/git_worktree.rs` — GitWorktree トレイト定義
- `docs/architecture/effects.md` — エフェクトの設計仕様
