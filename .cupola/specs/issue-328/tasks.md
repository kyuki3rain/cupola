# Implementation Plan

## Tasks Overview

テストのみの追加。プロダクションコードへの変更なし。全タスクは `devbox run test` で検証する。

---

- [ ] 1. stall_timeout 境界値単体テストの追加
- [ ] 1.1 (P) `find_stalled_with_zero_timeout` — timeout=0 で全セッションが stall 検出されることを検証する
  - `src/application/session_manager.rs` の `#[cfg(test)]` ブロックに追加
  - `spawn_sleep(60)` で複数のプロセスを register し、`find_stalled(Duration::ZERO)` が全件を返すことを assert
  - `Duration::ZERO` では `now.duration_since(started_at) > Duration::ZERO` が常に成立することをコメントで明記
  - _Requirements: 2.4, 2.5_

- [ ] 1.2 (P) `find_stalled_just_under_threshold` — 境界値 -1ns で stall 非検出を検証する
  - `spawn_sleep(60)` を register し、`find_stalled(Duration::MAX)` が空を返すことを assert
  - `Duration::MAX` は「到達不可能な timeout」として境界 -1ns の代替表現であることをコメントで明記
  - _Requirements: 2.2, 2.5_

- [ ] 1.3 (P) `find_stalled_just_over_threshold` — 境界値 +1ns で stall 検出を検証する
  - `spawn_sleep(60)` を register 直後に `find_stalled(Duration::from_nanos(1))` を呼び出す
  - `Instant::now()` の精度上ほぼ確実に stall 検出されることを assert
  - 検出される理由（経過時間 > 1ns が成立する）をコメントで明記
  - _Requirements: 2.3, 2.5_

- [ ] 1.4 (P) `find_stalled_at_exact_threshold` — ちょうど境界での動作（`>` 判定の仕様確定）を検証する
  - `find_stalled` が `>` (strictly greater than) 判定であることをコメントで明記
  - `Duration::ZERO` での全件検出と `Duration::MAX` での全件非検出を組み合わせ、境界の挙動を表現
  - Instant ベースでの「ちょうど境界」シミュレートが困難な理由をコメントで説明し、代替的な境界検証を提供する
  - _Requirements: 2.1, 2.5_

- [ ] 2. マルチ並行 issue 結合テストの追加
- [ ] 2.1 `five_concurrent_issues_isolated_progress` — 5 issue 並行処理の独立進行を検証する
  - `tests/integration_test.rs` に `#[tokio::test]` で追加
  - 5 件の Issue (DesignRunning state) と対応する ProcessRun(running) を in-memory SQLite に INSERT
  - `echo ""` プロセスを 5 件 spawn し、`SessionManager` に register
  - `std::thread::sleep(Duration::from_millis(300))` でプロセス終了を待機
  - `resolve_exited_sessions` を呼び出し、5 件全ての ProcessRun.state が Succeeded であることを assert
  - _Requirements: 1.1, 1.4, 1.5_

- [ ] 2.2 `concurrent_issues_db_write_accuracy` — 並行 issue の DB 書き込み正確性を検証する
  - `tests/integration_test.rs` に `#[tokio::test]` で追加
  - 3〜4 件の Issue を DesignRunning/ImplementationRunning 混在で INSERT
  - 各 issue に ProcessRun(running) + `echo ""` プロセスを register
  - `resolve_exited_sessions` 後、全 ProcessRun に state (Succeeded または Failed) が設定されていることを assert
  - `None` で persist されたレコードが残らないことを確認
  - コメントで「単一プロセス内では Mutex 直列化のため真の lock 競合は発生しないが、並行呼び出し後の正確性を検証する」と明記
  - _Requirements: 1.2, 1.4, 1.5_

- [ ] 2.3 `concurrent_session_limit_rejects_excess` — session 上限 reject の挙動を検証する
  - `tests/integration_test.rs` に `#[test]` (同期) または `#[tokio::test]` で追加
  - `sleep 60` プロセスを 2 件 register し `count() == 2` を確認
  - `try_reserve(2)` が `false` を返すことを assert (上限 = 2 に達している)
  - 5 issue 投入シナリオとして、上限到達後の 3 件が reject されることを検証
  - _Requirements: 1.3, 1.5_

- [ ] 3. T-RS.4 エンドツーエンドテストの追加
- [ ] 3.1 `stalled_session_is_killed_and_marked_failed_next_cycle` — stall → kill → failed の E2E 経路を検証する
  - `tests/integration_test.rs` に `#[tokio::test]` で追加
  - Issue・ProcessRun(running) を in-memory SQLite に INSERT
  - `sleep 60` プロセスを `SessionManager` に register
  - `kill_stalled(&mut session_mgr, 0)` を呼び出し (timeout=0 で即座に全件 stall 扱い)
  - `std::thread::sleep(Duration::from_millis(300))` でプロセス終了を待機
  - `resolve_exited_sessions` を呼び出し（次の polling サイクルをシミュレート）
  - `process_repo.find_latest(issue_id, ProcessRunType::Design)` で ProcessRun.state == Failed を assert
  - test-spec.md の T-RS.4 仕様への対応であることをコメントで明記
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 4. 品質チェックと修正
- [ ] 4.1 `devbox run clippy` でリントエラーがないことを確認し、警告を修正する
  - `#[allow(clippy::expect_used)]` は既存テストファイルのヘッダに設定済みであることを確認
  - 新規テスト関数内の `.expect()` 呼び出しに意図を示すメッセージを付与
  - _Requirements: 1.4, 2.5, 3.3_

- [ ] 4.2 `devbox run test` で全テストが green であることを確認する
  - 新規テスト 8 本（単体 4 本 + 統合 4 本）が全て pass することを確認
  - 既存テストへのリグレッションがないことを確認
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 2.4, 3.1, 3.2_
