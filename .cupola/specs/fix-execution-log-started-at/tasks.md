# Implementation Plan

## Task Format Template

---

- [x] 1. SessionManager に log_id 保持機能を追加する
- [x] 1.1 SessionEntry と ExitedSession に log_id フィールドを追加する
  - `SessionEntry`（private struct）に `log_id: i64` フィールドを追加する
  - `ExitedSession`（public struct）に `pub log_id: i64` フィールドを追加する
  - `register()` メソッドのシグネチャを `(issue_id: i64, child: Child, log_id: i64)` に変更する
  - `sessions.insert()` 時に `log_id` を `SessionEntry` に格納する
  - `log_id` は登録後に変更されない不変フィールドとして扱う
  - _Requirements: 3.1, 3.3, 3.4_

- [x] 1.2 collect_exited() が ExitedSession に log_id を含めて返すようにする
  - `results.push(ExitedSession { ... })` の構造体初期化に `log_id: entry.log_id` を追加する
  - `ExitedSession` の全構築箇所（テストのモック等）で `log_id` フィールドを初期化する
  - _Requirements: 3.2_

- [x] 1.3 SessionManager の既存ユニットテストを更新する
  - `register()` 呼び出しを新しいシグネチャ（`log_id` 引数付き）に合わせて更新する
  - `collect_exited()` の戻り値に `log_id` フィールドが含まれることを検証するアサーションを追加する
  - `log_id = 42` など具体的な値で渡して `ExitedSession.log_id == 42` を確認する
  - _Requirements: 3.1, 3.2, 3.4_

- [x] 2. PollingUseCase の実行ログ記録フローを修正する
- [x] 2.1 step7_spawn_processes でプロセス spawn 後に実行ログの開始を記録する
  - `spawn()` 成功後・`session_mgr.register()` 呼び出し前に `exec_log_repo.record_start(issue.id, issue.state).await.unwrap_or(0)` を呼ぶ
  - 取得した `log_id` を `session_mgr.register(issue.id, child, log_id)` に渡す
  - `record_start` が失敗した場合は `log_id = 0` でフォールバックし、spawn フロー自体は中断しない
  - PID 永続化処理（`issue_repo.update`）は変更しない
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 2.2 step3_process_exit_check から record_start を削除し session.log_id を使用する
  - `step3_process_exit_check` 内の `exec_log_repo.record_start()` 呼び出し（4行）を削除する
  - 削除した `log_id` 変数を `session.log_id` で置き換える
  - `handle_successful_exit` への `log_id` 引数は `session.log_id` を渡すよう変更する
  - 異常終了パスの `record_finish` 呼び出しも `session.log_id` を使うよう変更する
  - _Requirements: 1.4, 2.1, 2.2, 2.3, 2.4_

- [x] 3. 修正後の実行ログ記録フローを検証するテストを追加・更新する
- [x] 3.1 step7 で record_start が呼ばれ、step3 で record_start が呼ばれないことを検証する
  - `MockExecutionLogRepository` に `record_start` の呼び出し回数・引数を追跡する機能を追加する
  - spawn 成功時に `record_start` が1回呼ばれ、`issue_id` と `state` が正しいことを確認する
  - `step3_process_exit_check` 実行後に `record_start` の追加呼び出しがないことを確認する
  - `ExitedSession.log_id` が `record_start` から返された値と一致することを確認する
  - _Requirements: 1.1, 1.4, 2.4_

- [ ]* 3.2 started_at が finished_at より前になることを検証する統合テストを追加する
  - `record_start` と `record_finish` の呼び出し順序が `started_at < finished_at` を保証することを確認する
  - 正常終了・異常終了の両パスで `log_id` が一貫して使われることを確認する
  - 同一 Issue に複数回 spawn した場合に独立したログエントリが生成されることを確認する
  - _Requirements: 4.1, 4.2, 4.3_
