# Research & Design Decisions

---
**Purpose**: 調査結果・アーキテクチャ検討・設計決定の根拠を記録する。

---

## Summary

- **Feature**: `fix-execution-log-started-at`
- **Discovery Scope**: Extension（既存システムへのバグ修正）
- **Key Findings**:
  - `session_mgr.register()` の呼び出し箇所は `step7_spawn_processes` の1箇所のみ。シグネチャ変更の影響範囲は限定的。
  - `ExitedSession` は `collect_exited()` 内部でのみ生成される。`log_id` フィールド追加は構造的に安全。
  - `exec_log_repo.record_start()` は async trait メソッドであり、spawn 成功後に `await` して `log_id` を取得するフローは既存パターンと整合する。

## Research Log

### step7_spawn_processes のコード構造

- **Context**: `record_start` の移動先となる `step7_spawn_processes` の処理フローを確認
- **Sources Consulted**: `src/application/polling_use_case.rs:570-750`
- **Findings**:
  - `spawn()` 成功後に `self.session_mgr.register(issue.id, child)` を呼んでいる（行732）
  - 直後に `current_pid` を DB に永続化している（行735-739）
  - `record_start` を spawn 成功後・`register()` 前に呼ぶのが自然な挿入ポイント
- **Implications**: `record_start` → `register(issue.id, child, log_id)` の順序で呼ぶ

### step3_process_exit_check のコード構造

- **Context**: 削除対象の `record_start` 呼び出しと置き換え後のフローを確認
- **Sources Consulted**: `src/application/polling_use_case.rs:300-354`
- **Findings**:
  - 行320-324: `record_start` を呼んで `log_id` を取得
  - 行326以降: `exit_status.success()` 分岐で `record_finish` を呼ぶ
  - `record_finish` は `handle_successful_exit` と `step3` の失敗パスの2箇所で呼ばれる
- **Implications**: `session.log_id` を参照するように変更するだけで両パスが対応可能

### SessionManager の構造

- **Context**: `log_id` を保持させる `SessionEntry` と `ExitedSession` の構造を確認
- **Sources Consulted**: `src/application/session_manager.rs:1-115`
- **Findings**:
  - `SessionEntry` は `child, started_at, stdout_handle, stderr_handle` を持つ private struct
  - `register(&mut self, issue_id: i64, mut child: Child)` が現在のシグネチャ
  - `ExitedSession` は `issue_id, exit_status, stdout, stderr` を持つ public struct
  - `register()` 呼び出し元は `polling_use_case.rs` の1箇所のみ
- **Implications**: `register` に `log_id: i64` 引数を追加、`ExitedSession` に `pub log_id: i64` フィールドを追加するだけで完結する

### ExecutionLogRepository トレイト

- **Context**: `record_start` の戻り値型と呼び出し方法を確認
- **Sources Consulted**: `src/application/port/execution_log_repository.rs`
- **Findings**:
  - `record_start(issue_id: i64, state: State) -> impl Future<Output = Result<i64>>`
  - 失敗時は `unwrap_or(0)` でフォールバックが許容されている（既存パターン踏襲）
- **Implications**: spawn 後に `await.unwrap_or(0)` で取得した `log_id` を `register()` に渡す

## Architecture Pattern Evaluation

| オプション | 説明 | 利点 | リスク / 制約 |
|------------|------|------|---------------|
| A: log_id を SessionEntry に保持（採用） | `register()` の引数に `log_id` を追加し `SessionEntry` に格納、`ExitedSession` に公開フィールドとして返す | 変更箇所が最小限。データフローが明確。SessionManager の責務に適合 | なし |
| B: HashMap<i64, i64> で別管理 | `PollingUseCase` が `issue_id → log_id` マップを別途保持 | SessionManager を変更しない | 状態が分散し整合性管理が複雑になる |
| C: ExitedSession に option で持たせる | `log_id: Option<i64>` にする | 後方互換性 | 不要な Option。常に存在するため `i64` が正しい |

## Design Decisions

### Decision: `log_id を SessionEntry に保持し ExitedSession で返す`

- **Context**: spawn 時に取得した `log_id` を終了検知時まで安全に受け渡す方法が必要
- **Alternatives Considered**:
  1. `SessionEntry` に `log_id` フィールドを追加 — SessionManager 内で完結
  2. `PollingUseCase` に別途 `HashMap<i64, i64>` を持たせる — 状態分散
- **Selected Approach**: オプション1。`SessionEntry.log_id: i64` + `ExitedSession.log_id: i64`
- **Rationale**: SessionManager はセッションのライフサイクル情報をすべて保持する責務を持つ。`log_id` はそのセッションに対応する DB レコードの識別子であり、自然に SessionEntry の一部
- **Trade-offs**: `register()` のシグネチャ変更が必要だが、呼び出し元は1箇所のみで影響は最小
- **Follow-up**: `register()` の既存テストが `log_id` 引数を受け取るよう更新が必要

## Risks & Mitigations

- `register()` シグネチャ変更によるコンパイルエラー — 呼び出し元が1箇所のみなので修正コストは低い
- `record_start` が失敗した場合（`log_id = 0`）の `record_finish(0, ...)` — 既存の `unwrap_or(0)` パターンを踏襲する。`log_id = 0` は「記録なし」として DB 側で許容される

## References

- `src/application/polling_use_case.rs` — step3（行300-354）とstep7（行570-750）
- `src/application/session_manager.rs` — SessionManager 実装
- `src/application/port/execution_log_repository.rs` — ExecutionLogRepository トレイト定義
