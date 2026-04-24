# merge-main-before-spawn サマリ

## Feature
Fixing 状態 Issue の Claude Code spawn 前に `origin/{default_branch}` をフェッチ・マージし、agent の worktree を CI（PR×main の merge commit 上でテスト）と同じコード状態に揃える。他 PR の main マージに起因する「CI だけ壊れ agent は再現できず無限修正ループ」問題を解消する。

## 要件サマリ
- Fixing (`DesignFixing` / `ImplementationFixing`) の spawn 前に fetch → `git merge --no-edit origin/{default_branch}` を実行。
- クリーンマージ: 追加 causes なしで spawn 継続。
- コンフリクト: `FixingProblemKind::Conflict` を causes に追加、`has_merge_conflict = true` で worktree を merging 状態のまま spawn 継続、プロンプト先頭にコンフリクト解消指示（英語）を挿入。
- fetch 失敗・コンフリクト以外の merge 失敗: `warn` ログを出し、merge をスキップして spawn 継続。spawn 全体は決してブロックしない。

## アーキテクチャ決定
- **`GitWorktree` トレイトに `merge(wt, branch)` を新規追加** (採用): 既存の `fetch()` と対称、モック差し替え可能、Clean Architecture 準拠。代替（fetch 済みを流用・`merge()` 内で fetch 合体）は fetch 失敗とコンフリクトの区別が困難。
- **fetch と merge を別呼び出しにする**: 既存 `fetch()` 再利用 + step7 側で失敗理由別に分岐できる。
- **`has_merge_conflict: bool` フラグを `build_fixing_prompt` / `build_session_config` のパラメータに追加** (採用): 既存 `causes` は GitHub PR 上の問題種別であり、worktree ローカルな merging 状態と概念が異なるため `causes` への新バリアント追加案は不採用。
- **merging 状態の worktree をそのまま渡す**: `git merge --abort` せず agent にコンフリクト解消まで責任を持たせる。プロンプト冒頭に最優先の指示を置くことでリスクを軽減。
- **Graceful Degradation**: fetch/merge はベストエフォート。ネットワーク障害等での spawn ブロックを許容しない。
- **Fixing 状態のみに限定**: パフォーマンス・副作用影響を局所化。

## コンポーネント
- `application/port/git_worktree.rs` (`GitWorktree` trait): `merge(worktree_path, branch) -> Result<()>` 追加。
- `adapter/outbound/git_worktree_manager.rs` (`GitWorktreeManager`): `run_git_in_dir(wt, ["merge", "--no-edit", "origin/{branch}"])` で実装。非 0 終了が即 `Err` となりコンフリクト判別が不要。
- `application/polling_use_case.rs` (`step7_spawn_processes`): Fixing 分岐の worktree 確認直後に fetch+merge を挿入、`has_merge_conflict` を算出し `build_session_config` に渡す。
- `application/prompt.rs` (`build_fixing_prompt`, `build_session_config`): `has_merge_conflict: bool` パラメータ追加、英語のコンフリクト解消セクションを "Actions Required" より前に挿入。
- `domain/fixing_problem_kind.rs`: 既存 `FixingProblemKind::Conflict` を再利用。

## 主要インターフェース
```rust
pub trait GitWorktree: Send + Sync {
    fn fetch(&self) -> Result<()>;
    fn merge(&self, worktree_path: &Path, branch: &str) -> Result<()>;
}

fn build_fixing_prompt(
    issue_number: u64, pr_number: u64, language: &str,
    causes: &[FixingProblemKind], has_merge_conflict: bool,
) -> String;
```

## 学び / トレードオフ
- 既存 `run_git_in_dir` が非 0 終了で自動 `Err` 化するため、コンフリクト判別ロジックが不要で adapter 実装が極小。
- `has_merge_conflict` フラグはローカル変数のみで DB 永続化しないため、次サイクルで再評価される副作用が自然。
- シグネチャ変更が `build_session_config` の全呼び出し元に波及するが、現状 step7 のみなのでコスト低。
- `causes` 概念を汚染しないための bool フラグ分離は、将来の概念混在リスク回避として意義大。
