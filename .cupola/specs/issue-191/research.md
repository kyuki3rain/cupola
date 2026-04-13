# Research & Design Decisions

---
**Purpose**: Fixing フェーズにおけるマージエラー種別判定の調査・設計決定記録
---

## Summary
- **Feature**: `issue-191` — Fixing スポーン前マージエラーの種別判定
- **Discovery Scope**: Extension（既存コードの修正）
- **Key Findings**:
  - `GitWorktreeManager::merge()` は exit code 1 を `MergeConflictError`、それ以外を plain `anyhow::Error` として返す
  - `ProcessRun` 作成が現在マージ後に行われており、`mark_failed` 呼び出しのために順序変更が必要
  - `has_merge_conflict` フラグは `build_session_config` に既に渡せる仕組みがあり、TODO コメントで検出が未実装であることが示されている

## Research Log

### MergeConflictError の返し方

- **Context**: `merge()` が返すエラー型をどのように判別するかを調査
- **Sources Consulted**: `src/adapter/outbound/git_worktree_manager.rs` lines 63-86, `src/application/port/git_worktree.rs` lines 1-22
- **Findings**:
  - exit code 1 のとき `anyhow::Error::from(MergeConflictError)` を返す
  - 2+ のとき通常の `anyhow!()` を返す
  - `e.is::<MergeConflictError>()` または `e.downcast_ref::<MergeConflictError>()` で判別可能
- **Implications**: `match` ガードで `e.is::<MergeConflictError>()` を使うのが最もシンプル

### ProcessRun 作成タイミング

- **Context**: `mark_failed(run_id, ...)` を呼ぶには `run_id` が必要だが、現在 ProcessRun はマージの後に作成されている
- **Sources Consulted**: `src/application/polling/execute.rs` lines 535-605（`prepare_process_spawn` 関数）
- **Findings**:
  - 現在の順序: (1) merge → (2) build_session_config → (3) `ProcessRun::new_running()` → (4) `process_repo.save()` → (5) spawn
  - `mark_failed` を merge エラー時に呼ぶには (1) の前に (3)(4) を実施する必要がある
  - `update_pid` や spawn 失敗時の `mark_failed` はすでにこのパターンで実装済み（lines 586-601）
- **Implications**: ProcessRun の作成を merge より前に移動する。これにより merge エラー時も ProcessRun が残るが、`mark_failed` で適切な状態遷移が記録される

### has_merge_conflict フラグの伝達

- **Context**: `has_merge_conflict` フラグが `build_session_config` に渡されているが TODO になっている
- **Sources Consulted**: `src/application/polling/execute.rs` line 563, `src/application/prompt.rs` lines 32, 169-176
- **Findings**:
  - `build_session_config(..., has_merge_conflict: bool)` は既にシグネチャに含まれている
  - `prompt.rs` でコンフリクトセクションが挿入される仕組みも実装済み
  - 現在は `false` をハードコードしているだけ
- **Implications**: ローカル変数 `has_merge_conflict: bool` をマッチ結果から設定し、そのまま渡すだけで完結する

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク | 判定 |
|-----------|------|------|--------|------|
| A: ProcessRun を先に作成 | merge 前に ProcessRun を save し run_id を確保 | mark_failed で監査証跡を残せる | ProcessRun が失敗状態で残ることがある | ✅ 採用 |
| B: エラーログのみ（mark_failed なし） | merge 失敗を error ログのみで記録し Err 返却 | シンプル | 監査証跡なし、issue 要件を満たさない | ❌ 却下 |
| C: ProcessRun に has_merge_conflict フィールド追加 | ProcessRun にフラグを保存し後で参照 | 将来的な参照可能性 | 不要な DB スキーマ変更、issue 要件外 | ❌ スコープ外 |

## Design Decisions

### Decision: ProcessRun 作成順序の変更

- **Context**: merge エラー時に `mark_failed` を呼ぶために `run_id` が必要
- **Alternatives Considered**:
  1. ProcessRun を merge 前に作成（採用）
  2. mark_failed を呼ばずエラーログのみ
- **Selected Approach**: `ProcessRun::new_running()` および `process_repo.save()` をマージ処理ブロックの前に移動する
- **Rationale**: 既存の spawn 失敗時（lines 586-601）と同じパターンに統一され、一貫した監査証跡設計になる
- **Trade-offs**: マージ前に ProcessRun が作成されるため、merge 失敗時に Running 状態から Failed に遷移するレコードが残る。これは許容できる動作（監査目的）
- **Follow-up**: `prepare_process_spawn` 内の変数スコープが変わるため、run_id の参照箇所を統一的に確認する

### Decision: has_merge_conflict のローカル変数管理

- **Context**: `has_merge_conflict` を ProcessRun に保存するか、ローカルで管理するか
- **Alternatives Considered**:
  1. ローカル `bool` 変数で管理（採用）
  2. ProcessRun に `has_merge_conflict` フィールドを追加
- **Selected Approach**: `let has_merge_conflict: bool` をマッチ結果から設定し、`build_session_config` に直接渡す
- **Rationale**: DB スキーマ変更不要で最小限の変更。issue の TODO コメントが想定していた「ProcessRun から検出」は複雑すぎる
- **Trade-offs**: has_merge_conflict は単一セッション内でのみ使用される一時的な状態であり、永続化の必要性は低い

## Risks & Mitigations

- ProcessRun を先に作成することで、稀なケース（save 成功後に merge が panic）でも `mark_failed` が呼ばれずゾンビ ProcessRun が残る可能性 — これは既存の spawn 失敗パターンと同様であり、start_use_case の orphan 回収ロジックで対処済み
- `e.is::<MergeConflictError>()` は型消去された `anyhow::Error` への downcast に依存する — `MergeConflictError` は `std::error::Error` を実装しており anyhow の type tag で判別可能。`git_worktree_manager.rs` の実装も変更不要

## References

- `src/application/polling/execute.rs` — `prepare_process_spawn` 関数（修正対象）
- `src/application/port/git_worktree.rs` — `MergeConflictError` 定義
- `src/adapter/outbound/git_worktree_manager.rs` lines 63-86 — merge 実装
- `src/application/prompt.rs` lines 169-176 — `has_merge_conflict` 利用箇所
- `docs/architecture/effects.md` line 174 — マージコンフリクトは Claude Code が自力で解決する
