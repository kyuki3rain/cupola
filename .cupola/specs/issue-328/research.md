# Research & Design Decisions

---
**Purpose**: テスト追加に際してのコードベース調査・設計判断の記録。
---

## Summary

- **Feature**: issue-328 — マルチ並行 issue 結合テスト & stall_timeout 境界値テスト
- **Discovery Scope**: Extension（既存テストスイートへの追加）
- **Key Findings**:
  - 既存の並行テストは `two_concurrent_sessions_github_error_isolated_per_session`（2 issue 並行）のみ
  - `find_stalled` は `now.duration_since(entry.started_at) > timeout` (strictly greater) で判定
  - `kill_stalled` は `stall_timeout_secs` を秒単位で受け取り `Duration::from_secs` に変換する
  - 既存統合テストは mock GitHubClient + in-memory SQLite で構成
  - `SessionManager::try_reserve` / `release_reservation` で並行上限を管理

## Research Log

### 既存並行テストの調査

- **Context**: どの並行テストが既に存在するかを把握する
- **Sources Consulted**: `tests/integration_test.rs:619`, `src/application/session_manager.rs:605-640`
- **Findings**:
  - `two_concurrent_sessions_github_error_isolated_per_session`: 2 issue を同時処理し GitHub failure isolation を検証
  - `try_reserve_returns_false_when_sessions_at_limit`: SessionManager 単体で上限拒否を検証
  - 3 件以上の並行テスト、DB lock contention テストは未実装
- **Implications**: 新規テストは既存パターンを踏襲し、`MockGitHubClient` + `SqliteConnection::open_in_memory()` を使う

### find_stalled の境界値動作

- **Context**: `>=` か `>` かによって境界テストの期待値が変わる
- **Sources Consulted**: `src/application/session_manager.rs:436-443`
- **Findings**:
  - `now.duration_since(entry.started_at) > timeout` で判定
  - つまり `elapsed == timeout` では stall 検出 **しない**、`elapsed > timeout` で検出
  - 既存テスト `find_stalled_detects_old_processes` は `Duration::from_nanos(1)` と `Duration::from_secs(3600)` のみ
- **Implications**:
  - `find_stalled_at_exact_threshold` では「ちょうど境界は検出されない」を assert する
  - `find_stalled_just_over_threshold` では「境界+1ns で検出される」を assert する
  - ただし `Instant::now()` ベースなので「ちょうど境界」をシミュレートするのが困難 → テスト設計の工夫が必要

### kill_stalled の引数型

- **Context**: E2E テストで `kill_stalled` を呼ぶ際の引数型を確認
- **Sources Consulted**: `src/application/polling/resolve.rs:582-590`
- **Findings**:
  - `pub fn kill_stalled(session_mgr: &mut SessionManager, stall_timeout_secs: u64)`
  - 引数は秒単位の `u64`。`Duration::from_secs(0)` で全件 stall 扱い
- **Implications**: E2E テストでは `stall_timeout_secs = 0` を渡して即座に全セッションを stall 扱いにできる

### DB lock contention の再現可能性

- **Context**: SQLite WAL モードで複数タスクが並行 write した場合の挙動
- **Sources Consulted**: `src/adapter/outbound/sqlite_connection.rs`, tech.md
- **Findings**:
  - `rusqlite` は `Arc<Mutex<Connection>>` で直列化される
  - 真の DB lock contention（busy_timeout に当たるシナリオ）は単一プロセス内では発生しない
  - 複数タスクが並行して `resolve_exited_sessions` を呼んでも Mutex で直列化される
- **Implications**:
  - `concurrent_issues_db_lock_contention_resolves` テストは「並行処理後も全 issue が correctly persist されること」を検証するテストとして実装する（真の lock 競合ではなく、並行呼び出しの結果の正確性を検証）

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク/制約 |
|-----------|------|------|-------------|
| 既存パターン踏襲 | `MockGitHubClient` + in-memory SQLite + `SessionManager::new()` を使った tokio テスト | コードベースとの一貫性、レビューコスト低 | なし |
| 新規 mock 導入 | 並行専用の mock を作る | より精密な制御 | 既存パターンとの乖離、実装コスト増 |

→ **既存パターン踏襲** を採用。

## Design Decisions

### Decision: stall_timeout 境界値テストの配置場所

- **Context**: 単体テストか統合テストか
- **Alternatives Considered**:
  1. `tests/integration_test.rs` に配置
  2. `src/application/session_manager.rs` の `#[cfg(test)]` に配置
- **Selected Approach**: `SessionManager::find_stalled` の境界値は `session_manager.rs` の `#[cfg(test)]` 内に配置する。`kill_stalled` の E2E テストは `tests/integration_test.rs` に配置する。
- **Rationale**: `find_stalled` は SessionManager のメソッドであり、単体テストとして SessionManager のモジュール内に配置するのが Clean Architecture の慣習に合う。E2E 経路は統合テストとして外部ファイルに配置する。
- **Trade-offs**: 境界値テストがファイルをまたぐが、それぞれの責務範囲が明確になる

### Decision: 境界値の時間制御方法

- **Context**: `find_stalled` は `Instant::now()` を使うため、ちょうど境界をシミュレートするのが困難
- **Alternatives Considered**:
  1. `std::thread::sleep` で経過時間を作る（不安定）
  2. `SessionManager` に時刻注入機構を追加する（変更コスト大）
  3. `Duration::ZERO` と `Duration::from_nanos(1)` を組み合わせて境界動作を近似的に検証する
- **Selected Approach**: `Duration::ZERO` での全件検出と `Duration::MAX` での全件非検出、および `Duration::from_nanos(1)` で「超短 timeout なら新規登録直後でも stall 検出」を組み合わせる。厳密な「ちょうど境界」は現実的に難しいため、`>` 判定の仕様をコメントで明記する。
- **Rationale**: `Instant` の精度依存を避けつつ、仕様（`>` 判定）を明確にコメントで表現できる
- **Trade-offs**: ちょうど境界の精密なテストは難しいが、仕様の意図（strictly greater than）を文書化できる

## Risks & Mitigations

- **Flaky risk (sleep 依存)**: `std::thread::sleep` を最小限に抑え、`Duration::from_nanos(1)` など微小 timeout を使う — 既存テストも同様のアプローチ
- **DB contention テストの意味論**: 真の SQLite lock 競合は単一プロセス内では起きない — テストの意図を「並行呼び出し後の正確性検証」として明記する
- **5 並行テストの複雑さ**: `tokio::join!` または `futures::join_all` を使い、セットアップを共通化してコードの重複を避ける

## References

- `docs/tests/test-spec.md` — T-RS.4, T-SM.* 仕様
- `tests/integration_test.rs:619` — 既存 2 issue 並行テストのパターン
- `src/application/session_manager.rs:436` — `find_stalled` の実装
- `src/application/polling/resolve.rs:582` — `kill_stalled` の実装
