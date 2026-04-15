# Research & Design Decisions

---
**Purpose**: Fixing spawn における fetch 欠如バグの調査・設計判断の記録。

---

## Summary

- **Feature**: `issue-250` — Fixing spawn 前の `fetch()` 追加
- **Discovery Scope**: Simple Addition（既存コードへの 1 関数呼び出し追加）
- **Key Findings**:
  - `worktree.fetch()` は `GitWorktree` トレイトに既に定義済みで、`SpawnableGitWorktree` 経由でも利用可能
  - SpawnInit（`perform_init_sync`）では `worktree.fetch()` を呼んでいるが、Fixing spawn（`spawn_process`）では呼ばれていない
  - Git worktree は `.git` を共有するため、`fetch()` によって `origin/main` ref は全 worktree に反映される
  - fetch 失敗は fatal にしない（既存の merge 失敗の warn 継続と対称的）

## Research Log

### Fixing spawn のコードパス

- **Context**: `execute.rs:583` の `DesignFix`/`ImplFix` 分岐を調査
- **Sources Consulted**: `src/application/polling/execute.rs`, `src/application/port/git_worktree.rs`
- **Findings**:
  - `spawn_process` 関数（`execute.rs:560`〜）が Fixing spawn を担当
  - `if matches!(type_, ProcessRunType::DesignFix | ProcessRunType::ImplFix)` ブロック（行 583）内で `worktree.merge()` のみ呼ばれており、`worktree.fetch()` は呼ばれていない
  - `GitWorktree` トレイト（`port/git_worktree.rs:12`）に `fn fetch(&self) -> Result<()>` が定義済み
  - `SpawnableGitWorktree` は `GitWorktree + Clone + 'static` のブランケット実装（`execute.rs:31-32`）であり、`fetch()` を利用可能
- **Implications**: 1 行 `worktree.fetch()` 呼び出し＋ warn ロギングを追加するだけで修正可能

### fetch 失敗時のエラー方針

- **Context**: fetch 失敗を fatal にするかどうかの判断
- **Findings**:
  - Issue 記述の Fix sketch に「fetch 失敗は fatal にせず `warn` で継続」と明示
  - 既存の `worktree.merge()` 失敗ハンドリング（`tracing::warn!` + 処理継続）と対称的
  - fetch 失敗時は古い ref のまま merge（現状と同等）になるため、リグレッションなし
- **Implications**: `if let Err(e) = worktree.fetch()` で warn ログを出し、merge ステップへ進む

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| fetch を fatal エラーとして扱う | fetch 失敗時に spawn を中断 | 古い ref での merge を完全排除 | fetch は一時的なネットワーク障害でも失敗し得る。CI_FIX_COUNT を不必要に消費する | 採用しない |
| fetch を warn で継続（採用） | 失敗時はログを残して merge を続行 | 既存の error handling と対称的、リグレッションなし | 古い ref での merge が残るが、ネットワーク正常時は解消する | Issue の Fix sketch と合致 |

## Design Decisions

### Decision: fetch 失敗を warn で継続

- **Context**: fetch は外部ネットワークに依存するため失敗する可能性がある
- **Alternatives Considered**:
  1. fetch 失敗時に spawn を abort — 安全だが不必要な Cancelled を増やす
  2. fetch 失敗時を warn で継続（採用）— 既存の merge warn と対称、現状と同等の fallback
- **Selected Approach**: `if let Err(e) = worktree.fetch() { tracing::warn!(...) }` でログ出力後、merge を続行
- **Rationale**: Issue の Non-goals にも「非 conflict merge エラーの audit trail は本 Issue で扱わない」と明示されている。fetch の warn も同様のポリシーと整合する
- **Trade-offs**: fetch 失敗時は古い ref での merge が残るが、正常時の修正効果は十分
- **Follow-up**: 実装後、統合テストでモック worktree を使って fetch → merge 順序を確認する

## Risks & Mitigations

- `worktree.fetch()` がモックで未実装の場合にテストが通らない可能性 → モック実装に `fetch` を追加する
- 既存テストが `spawn_process` 内の fetch 呼び出しを想定していない場合に失敗する可能性 → 既存テストの mock 期待値を更新する

## References

- `src/application/polling/execute.rs:583` — Fixing spawn 内の merge 呼び出し箇所
- `src/application/port/git_worktree.rs:12` — `GitWorktree::fetch` トレイト定義
- `src/application/polling/execute.rs:432` — SpawnInit での `worktree.fetch()` 呼び出し（参考実装）
