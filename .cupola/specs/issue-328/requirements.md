# Requirements Document

## Introduction

ポーリングループ (Resolve → Collect → Decide → Persist → Execute) の結合テストにおいて、現状カバーされていない 2 つのギャップを埋める。

1. **マルチ issue 並行テスト**: 現状は最大 2 issue 並行まで。3〜5 件の並行処理における race window・リソース競合 (DB lock, session_mgr 上限) を検証する。
2. **stall_timeout 境界値テスト**: `T-RS.4` 仕様 (`docs/tests/test-spec.md`) に対応する境界値網羅 (timeout-1ms / timeout / timeout+1ms / 0 / max) を追加する。

対象は `tests/integration_test.rs` および `src/application/session_manager.rs` の `#[cfg(test)]` ブロック。すべてのテストは `--test-threads=1` を要求せず、独立実行可能であること。

---

## Requirements

### Requirement 1: マルチ並行 issue 結合テスト

**Objective:** テストエンジニアとして、3〜5 件の issue が同時に処理される状況を統合テストで検証したい。それにより、並行時の race window・session_mgr 上限・DB lock 競合を回帰テストで捕捉できるようにする。

#### Acceptance Criteria

1.1. When 5 件の issue が異なる State で同時に `resolve_exited_sessions` に投入されたとき、the テストシステム shall 各 issue の exited session を逐次処理し、対応する ProcessRun が期待する State (succeeded/failed) へ遷移して最終的に persist されることを検証する。

1.2. When 複数の issue が並行して `resolve_exited_sessions` を実行したとき、the テストシステム shall DB lock contention が発生しても全 issue の ProcessRun が最終的に persist (succeeded または failed) されることを検証する。

1.3. When `max_concurrent_sessions = 2` で 5 件の issue を `spawn_process` 経由で session_mgr に登録しようとしたとき、the テストシステム shall `SessionManager::try_reserve` が上限到達後に `false` を返し、超過分の issue がセッション確保に失敗することを検証する。

1.4. The テストシステム shall 並行テストを `tokio::join!` または `Vec<JoinHandle>` で構成し、`--test-threads=1` を要求しない独立実行可能な形式で実装する。

1.5. The テストシステム shall 並行テストで mock GitHubClient を使用し、実際の GitHub API 呼び出しを行わない。

### Requirement 2: stall_timeout 境界値テスト

**Objective:** テストエンジニアとして、`find_stalled` の境界値を精密に網羅するテストを追加したい。それにより、T-RS.4 仕様 (`docs/tests/test-spec.md`) に定義された stall 検出の境界条件が実装で正しく扱われることを保証する。

#### Acceptance Criteria

2.1. When session の経過時間が stall_timeout ちょうど (`==`) のとき、the SessionManager shall その session を stall として検出する (または検出しない) かを明示的に検証し、`>` か `>=` かの境界動作を確定する。

2.2. When session の経過時間が `stall_timeout - 1ms` のとき、the SessionManager shall その session を stall として検出しないことを検証する。

2.3. When session の経過時間が `stall_timeout + 1ms` のとき、the SessionManager shall その session を stall として検出することを検証する。

2.4. When `stall_timeout = Duration::ZERO` のとき、the SessionManager shall 登録された全ての session を stall として検出することを検証する。

2.5. The テストシステム shall 境界値テストを `src/application/session_manager.rs` の `#[cfg(test)]` ブロック内に配置し、単体テストとして独立実行可能にする。

### Requirement 3: T-RS.4 エンドツーエンドテスト

**Objective:** テストエンジニアとして、stall 検出から SIGKILL 送信・次サイクルでの failure 処理までの経路を統合テストで検証したい。それにより、T-RS.4 仕様のエンドツーエンド動作が regression から保護される。

#### Acceptance Criteria

3.1. When session が登録されてから stall_timeout を超過し `kill_stalled` が呼ばれたとき、the テストシステム shall 対象プロセスに SIGKILL が送信され、プロセスが終了することを検証する。

3.2. When `kill_stalled` によって終了したセッションが次の `resolve_exited_sessions` サイクルで処理されたとき、the テストシステム shall `ProcessRun.state` が `failed` になることを検証する。

3.3. The テストシステム shall stall → kill → failed の経路を `tests/integration_test.rs` 内の非同期統合テストとして実装し、`--test-threads=1` を要求しない形式にする。
