# Research & Design Decisions

## Summary
- **Feature**: `worktree-origin-start-point`
- **Discovery Scope**: Extension（既存システムの拡張）
- **Key Findings**:
  - `initialize_issue` メソッド内で `self.worktree.create(wt, &main_branch, "HEAD")` を使用しており、ここが修正対象
  - `GitWorktree` trait に `fetch` メソッドが未定義。新規追加が必要
  - `self.config.default_branch` で設定値にアクセス可能。`format!("origin/{}", self.config.default_branch)` で起点を構成できる

## Research Log

### GitWorktree trait の現状分析
- **Context**: fetch 操作を追加するための拡張ポイントの調査
- **Sources Consulted**: `src/application/port/git_worktree.rs`, `src/adapter/outbound/git_worktree_manager.rs`
- **Findings**:
  - 現在の trait メソッド: `create`, `remove`, `create_branch`, `checkout`, `pull`, `push`, `delete_branch`
  - `pull` は worktree_path 内で実行するが、`fetch` はリポジトリルートで実行する必要がある
  - `GitWorktreeManager` は `repo_root` フィールドを持ち、`run_git` メソッドでリポジトリルートからコマンドを実行可能
- **Implications**: `fetch` メソッドは `run_git` を使用してリポジトリルートで実行する。trait に `fn fetch(&self) -> Result<()>` を追加

### initialize_issue の修正範囲
- **Context**: worktree 作成の起点変更の影響範囲調査
- **Sources Consulted**: `src/application/polling_use_case.rs:231-249`
- **Findings**:
  - `initialize_issue` は `PollingUseCase` のプライベートメソッド
  - `self.config` で `Config` にアクセス可能（`default_branch` フィールド含む）
  - 修正は2箇所: (1) fetch 呼び出しの追加、(2) start_point の変更
  - worktree が既に存在する場合は `create` が早期リターンするため、fetch の実行順序に注意が必要
- **Implications**: fetch は create の前に呼び出す。既存の worktree がある場合でも fetch は実行して問題ない

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| GitWorktree trait に fetch 追加 | 既存 trait を拡張 | 一貫した抽象化、テスト容易 | trait の肥大化 | 推奨。fetch は worktree 操作の前提条件 |
| 別の GitFetcher trait を定義 | fetch 専用の trait を分離 | ISP 準拠 | 過度な抽象化、DI の複雑化 | 不採用。単一メソッドのためオーバーエンジニアリング |

## Design Decisions

### Decision: GitWorktree trait に fetch メソッドを追加
- **Context**: リモートからの最新取得を抽象化する必要がある
- **Alternatives Considered**:
  1. GitWorktree trait に `fn fetch(&self) -> Result<()>` を追加
  2. 新規 `GitFetcher` trait を定義して PollingUseCase に注入
- **Selected Approach**: Option 1 — GitWorktree trait に追加
- **Rationale**: fetch は worktree 操作の前提条件であり、同じ trait に含めるのが自然。GitWorktreeManager は既に `run_git` メソッドを持ち、リポジトリルートでのコマンド実行が可能
- **Trade-offs**: trait が少し大きくなるが、7→8 メソッドで許容範囲。新しい依存関係の注入が不要
- **Follow-up**: mock 実装の更新が必要（テストファイルの確認）

### Decision: start_point を origin/{default_branch} に変更
- **Context**: ローカル HEAD ではなくリモートの最新を起点にする
- **Selected Approach**: `format!("origin/{}", self.config.default_branch)` で起点文字列を生成
- **Rationale**: `self.config.default_branch` は既に利用可能。PR 作成時にも同フィールドを参照している（polling_use_case.rs:584）
- **Trade-offs**: リモートが到達不能な場合は fetch エラーで中断されるが、これは期待される動作

## Risks & Mitigations
- ネットワーク障害で fetch 失敗 → エラーを返して worktree 作成を中断（要件 1.2 に準拠）
- 既存テストの mock が fetch メソッドを実装していない → mock に空実装を追加
