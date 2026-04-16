# Implementation Tasks: issue-258 — StopUseCase PID ファイルリーク修正

## Task Overview

- 修正対象: `src/application/stop_use_case.rs`（2 箇所の `?` によるエラー伝播を置き換え）
- テスト追加: 同ファイルの `#[cfg(test)]` モジュール内に 3 件のテストを追加
- 新ファイル・新型・依存関係追加: なし

---

- [ ] 1. シグナル失敗パスの PID ファイルクリーンアップ実装

- [ ] 1.1 SIGTERM エラーパスの修正
  - `self.signal.send_sigterm(pid)?;`（line 42）を `if let Err(e) = self.signal.send_sigterm(pid)` ブロックに置き換える
  - ブロック内で `self.pid_file.is_process_alive(pid)` を確認し、`false`（プロセス終了確認）の場合のみ `delete_pid()` を呼び出す
  - `delete_pid()` が失敗した場合は `tracing::warn!` でログ記録し、エラーは swallow する
  - ブロック末尾で `return Err(e)` により元のシグナルエラーを返す
  - _Requirements: 1.1, 1.3, 1.4_

- [ ] 1.2 SIGKILL エラーパスの修正
  - `self.signal.send_sigkill(pid)?;`（line 59）を `if let Err(e) = self.signal.send_sigkill(pid)` ブロックに置き換える
  - 1.1 と同様のロジック（`is_process_alive` チェック → `delete_pid` クリーンアップ → `return Err(e)`）
  - 既存の `if self.pid_file.is_process_alive(pid)` チェック（line 64）および `delete_pid()` 呼び出し（line 70）は変更しない
  - _Requirements: 1.2, 1.3, 1.4_

- [ ] 2. シグナル失敗パスのテスト追加

- [ ] 2.1 (P) SIGTERM 失敗 + プロセス死亡シナリオのテスト
  - `MockPidFile` に `deleted: Arc<Mutex<bool>>` フィールドを追加し、`delete_pid()` が呼ばれた際に `true` に設定する変形 mock を作成する（テスト用ローカル構造体として定義）
  - `MockSignal` で `send_sigterm()` が `StopError::Signal(...)` を返す変形 mock を作成する
  - `is_process_alive()` が `false` を返す（プロセス死亡）設定で `execute()` を呼び出す
  - `execute()` の結果が `Err` であること、かつ `deleted` フラグが `true` であることをアサートする
  - テスト名: `sigterm_failure_with_dead_process_deletes_pid_file`
  - _Requirements: 3.1_

- [ ] 2.2 (P) SIGKILL 失敗 + プロセス死亡シナリオのテスト
  - 2.1 と同様の変形 mock を使用する
  - `MockSignal` で `send_sigterm()` は成功するが `send_sigkill()` が `StopError::Signal(...)` を返す設定にする
  - タイムアウトを極短（`Duration::from_millis(1)`）に設定し SIGKILL パスに誘導する
  - `is_process_alive()` は SIGTERM 送信後も `true`（SIGTERM 無視）、SIGKILL 送信後は `false` を返す設定
  - `execute()` の結果が `Err` であること、かつ `deleted` フラグが `true` であることをアサートする
  - テスト名: `sigkill_failure_with_dead_process_deletes_pid_file`
  - _Requirements: 3.2_

- [ ] 2.3 SIGTERM 失敗 + プロセス生存シナリオのテスト
  - `send_sigterm()` が Err を返し、`is_process_alive()` が `true`（プロセス生存）を返す設定
  - `execute()` の結果が `Err` であること、かつ `deleted` フラグが `false`（delete_pid 未呼び出し）であることをアサートする
  - テスト名: `sigterm_failure_with_alive_process_does_not_delete_pid_file`
  - _Requirements: 3.3_

- [ ] 3. 品質確認

- [ ] 3.1 既存テスト (T-6.SP.*) のリグレッション確認
  - `devbox run test` を実行し全テストが通過することを確認する
  - `devbox run clippy` を実行し警告・エラーがないことを確認する
  - `devbox run fmt-check` を実行しフォーマット違反がないことを確認する
  - _Requirements: 2.1, 2.2, 2.3_
