# Research & Design Decisions

---
**Purpose**: merge-main-before-spawn の設計調査と意思決定の記録

---

## Summary
- **Feature**: `merge-main-before-spawn`
- **Discovery Scope**: Extension（既存システムへの機能追加）
- **Key Findings**:
  - `GitWorktree` トレイトは既に `fetch()` を持つ。`merge()` メソッドを追加するだけで整合する。
  - `GitWorktreeManager.run_git_in_dir()` を使えばworktreeディレクトリ内でのgitコマンド実行は既存パターンで実現可能。
  - `git merge --no-edit` の終了コードでクリーン/コンフリクトを区別できる（コンフリクト時は非0）。
  - `build_fixing_prompt` は `causes: &[FixingProblemKind]` を受け取る純粋関数。フラグ引数を追加するだけで対応可能。

## Research Log

### GitWorktree トレイトの既存設計
- **Context**: mergeメソッドをどこに追加するか確認
- **Sources Consulted**: `src/application/port/git_worktree.rs`, `src/adapter/outbound/git_worktree_manager.rs`
- **Findings**:
  - トレイトには `fetch(&self)`, `pull(wt, ...)`, `push(wt, ...)` 等が存在する
  - `run_git_in_dir(wt_path, args)` ヘルパーがすでにあり、worktree内でのgitコマンドを実行する標準パターン
  - `fetch()` はリポジトリルートで実行するが、`merge()` はworktreeディレクトリ内で実行する必要がある（`run_git_in_dir` を使用）
- **Implications**: `merge(wt: &Path, branch: &str) -> Result<()>` の実装は `run_git_in_dir` で直接実現できる

### git merge のコンフリクト検出
- **Context**: コンフリクト発生を確実に検出する方法
- **Sources Consulted**: git documentation（既存知識）
- **Findings**:
  - `git merge --no-edit <branch>` はコンフリクト時に非0終了コードを返す
  - `run_git_in_dir` は非0終了時に `Err` を返すため、既存パターンがそのまま使える
  - stderrに "CONFLICT" が含まれるが、終了コードだけで判別可能
- **Implications**: adapterの実装はシンプルに `run_git_in_dir` を呼び出すだけでよい。コンフリクト判別はエラー返りで済む

### step7_spawn_processes の既存フロー
- **Context**: fetch+mergeをどこに挿入するか
- **Sources Consulted**: `src/application/polling_use_case.rs` step7実装（行607〜）
- **Findings**:
  - DesignRunning/ImplementationRunning に対してPR状態チェックが先に行われる（継続ロジック）
  - Fixing状態は上記チェックをスキップし、worktree_pathの存在確認 → inputs書き込み → spawn へ進む
  - worktree_pathの確認後、`index.lock` クリーンアップが入っている
  - fetch+mergeはworktree確認の直後（inputs書き込み前）に挿入するのが自然な位置
- **Implications**: Fixing状態の `pr_number_to_check == None` のパスに入る直後（worktree確認後）にmergeロジックを追加する

### build_fixing_prompt の既存実装
- **Context**: コンフリクト解消指示をどう追加するか
- **Sources Consulted**: `src/application/prompt.rs`（行178〜257）
- **Findings**:
  - 関数シグネチャ: `fn build_fixing_prompt(issue_number, pr_number, language, causes) -> String`
  - `causes.contains(&FixingProblemKind::Conflict)` の分岐は既に存在するが、指示は末尾のリストに追加されるだけ
  - プロンプト先頭への挿入には `has_merge_conflict: bool` フラグを追加するのが最もシンプル
  - `build_session_config` 経由で呼ばれるため、そちらにもフラグを渡す必要がある
- **Implications**: `build_fixing_prompt` のシグネチャ変更は `build_session_config` にも波及する。呼び出し元は `step7_spawn_processes` 内

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク | 採用 |
|-----------|------|------|--------|------|
| トレイトにmergeメソッド追加 | `GitWorktree` に `merge(wt, branch)` を追加 | Clean Architecture準拠、モック可能、既存パターン踏襲 | なし | ✅ 採用 |
| fetch済みをmergeに流用 | `fetch()` をstep7で再呼び出しせずmergeのみ | fetchとmergeを分離できる | fetchとmergeの間に状態不整合リスク | 採用せず |
| fetchをmerge内に含める | `merge()` 内でfetch+mergeを一括実行 | 呼び出し側がシンプル | fetch失敗とmergeコンフリクトの区別が困難 | 採用せず |

## Design Decisions

### Decision: merge失敗タイプの区別方法
- **Context**: fetchエラー vs コンフリクトエラーをapplication層でどう区別するか
- **Alternatives Considered**:
  1. エラー型を enum で区別（`MergeError::Conflict`, `MergeError::NetworkError` 等）
  2. 単純な `Result<()>` で返し、adapterがコンフリクト時のみ Err を返す（fetchはadapter外で別途呼ぶ）
- **Selected Approach**: fetch失敗は `fetch()` の Err、mergeコンフリクトは `merge()` の Err として分離する
- **Rationale**: 既存の `fetch()` をそのまま再利用できる。step7でfetch失敗時はwarnして継続、merge失敗（コンフリクト）時はcauses追加という分岐が明確になる
- **Trade-offs**: 2回の呼び出しになるが、fetch/mergeの失敗理由の区別が容易
- **Follow-up**: fetchとmergeの間に他のPRからのpushが来た場合は次のポーリングサイクルで再処理される

### Decision: has_merge_conflictフラグの渡し方
- **Context**: `build_fixing_prompt` にコンフリクト状態をどう伝えるか
- **Alternatives Considered**:
  1. `bool` フラグを追加パラメータとして渡す
  2. `causes` に `MergeConflict` バリアントを追加
- **Selected Approach**: `bool` フラグ (`has_merge_conflict: bool`) を追加
- **Rationale**: `causes` はGitHub上のPR状態を表し、worktreeのローカル状態（merging中）とは概念が異なる。ローカルのみのフラグを `causes` に混在させるべきでない
- **Trade-offs**: 引数が増えるが概念の分離は明確
- **Follow-up**: `build_session_config` のシグネチャにも `has_merge_conflict: bool` を追加する必要がある

## Risks & Mitigations
- worktreeがmerging状態でagentがspawnされると、agentがコンフリクト解消前に別の作業を始めるリスク — プロンプト先頭にコンフリクト解消指示を記載し、最初に対応させる
- fetch/mergeを毎回のspawn前に実行するとパフォーマンスへの影響が生じる可能性 — Fixing状態のみに限定しているため影響は最小限
- `git merge --no-edit` 実行後のworktreeがmerging状態のままspawnがスキップされる場合、次のポーリングでも再度mergeを試みる — worktreeはmerging状態なので `git merge --continue` 等が必要だが、agentに解決させることが前提のため許容範囲

## References
- `src/application/port/git_worktree.rs` — 既存GitWorktreeトレイト定義
- `src/adapter/outbound/git_worktree_manager.rs` — トレイトの具体実装
- `src/application/polling_use_case.rs` — step7_spawn_processes の実装
- `src/application/prompt.rs` — build_fixing_prompt の実装
- `src/domain/fixing_problem_kind.rs` — FixingProblemKind enum
