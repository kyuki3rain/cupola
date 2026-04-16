# Requirements Document

## Introduction

`stop_use_case.rs` において `send_sigterm()` または `send_sigkill()` がエラーを返した際、即座に `Err` を返すため `delete_pid()` が呼ばれずステール PID ファイルが残留するバグを修正する。ステール PID ファイルが残ると次回の `cupola start` が起動を拒否するという運用上の重大問題につながる。

対象ファイル: `src/application/stop_use_case.rs`（lines 42〜72）

## Requirements

### Requirement 1: シグナル送信失敗時の PID ファイルクリーンアップ

**Objective:** デーモン管理者として、シグナル送信が失敗した場合でもプロセスの終了が確認できれば PID ファイルが確実に削除されることを望む。これにより、ステール PID ファイルの残留によって次回のデーモン起動がブロックされないようにするため。

#### Acceptance Criteria

1. When `send_sigterm()` がエラーを返し、かつ `is_process_alive()` がプロセスの終了を確認した場合、StopUseCase shall `delete_pid()` を呼び出してからエラーを返す
2. When `send_sigkill()` がエラーを返し、かつ `is_process_alive()` がプロセスの終了を確認した場合、StopUseCase shall `delete_pid()` を呼び出してからエラーを返す
3. If シグナル送信失敗後も `is_process_alive()` がプロセスの生存を示している場合、StopUseCase shall PID ファイルを削除せずに元のシグナルエラーをそのまま返す
4. If クリーンアップパスでの `delete_pid()` がエラーを返した場合、StopUseCase shall そのエラーをログに記録し、元のシグナルエラーを優先して返す

### Requirement 2: 既存の停止フローの保持

**Objective:** デーモン管理者として、修正後も正常な SIGTERM / SIGKILL 停止フローが引き続き正しく動作することを望む。これにより、修正がリグレッションを引き起こさないようにするため。

#### Acceptance Criteria

1. When `send_sigterm()` が成功し、その後プロセスが終了した場合、StopUseCase shall `StopResult::Stopped` を返し PID ファイルを削除する
2. When タイムアウト後に `send_sigkill()` を送信してプロセスが終了した場合、StopUseCase shall `StopResult::ForceKilled` を返し PID ファイルを削除する
3. The existing T-6.SP.* テスト shall 修正後も引き続き全件通過する

### Requirement 3: シグナル失敗パスのテスト追加

**Objective:** 開発者として、シグナル送信失敗時の PID クリーンアップ動作を単体テストで検証したい。これにより、今後のリグレッションを防ぐため。

#### Acceptance Criteria

1. When `send_sigterm()` が EPERM に相当するエラーを返し、かつプロセスが既に死亡している場合、テスト shall `delete_pid()` が呼び出されることを検証する
2. When `send_sigkill()` がエラーを返し、かつプロセスが既に死亡している場合、テスト shall `delete_pid()` が呼び出されることを検証する
3. When `send_sigterm()` がエラーを返し、かつプロセスがまだ生存している場合、テスト shall `delete_pid()` が呼び出されないことを検証する
