# Requirements Document

## Project Description (Input)
cupola start/stop コマンドの導入（デーモン化対応）: cupola run を cupola start にリネームし、デーモン化オプション（-d）と cupola stop コマンドを追加する機能

## Introduction

cupola は GitHub Issues をポーリングし、自動設計・実装を行うローカル常駐エージェントである。
現在の `cupola run` コマンドはフォアグラウンドで動作するが、ターミナルマルチプレクサ（zellij）経由でのシグナル配信が不安定になる問題がある（スリープ復帰後に Ctrl+C が無効化される）。

本機能では `cupola run` を `cupola start` にリネームし、`-d/--daemon` オプションによるバックグラウンド（デーモン）起動機能と、`cupola stop` コマンドによる確実な停止機能を追加する。
デーモン起動時は `.cupola/cupola.pid` に PID を記録し、二重起動を防止するとともに `cupola stop` での確実な停止を可能にする。

## Requirements

### Requirement 1: `cupola run` から `cupola start` へのリネーム

**Objective:** cupola ユーザーとして、`cupola start` コマンドでポーリングループをフォアグラウンド起動したい。これにより、既存の `cupola run` の動作を維持しつつ、より意味のあるコマンド名で操作できる。

#### Acceptance Criteria

1. The cupola CLI shall provide a `start` subcommand that replaces the existing `run` subcommand.
2. The cupola CLI shall remove the `run` subcommand entirely upon implementation of `start`.
3. When `cupola start` is executed without the `-d` flag, the cupola CLI shall launch the polling loop in the foreground with the same behavior as the former `run` command.
4. The cupola CLI shall accept the same options as the former `run` command (`--polling-interval-secs`, `--log-level`, `--config`) for the `start` subcommand.
5. When `cupola start` is executed in the foreground, the cupola CLI shall terminate the polling loop upon receiving SIGTERM or SIGINT (Ctrl+C).

---

### Requirement 2: `cupola start -d` によるデーモン起動

**Objective:** cupola ユーザーとして、`cupola start -d` でデーモン（バックグラウンド）として起動したい。これにより、ターミナルを閉じてもエージェントが動作し続け、スリープ復帰後でも確実に停止できる。

#### Acceptance Criteria

1. The cupola CLI shall accept a `-d` / `--daemon` flag for the `start` subcommand.
2. When `cupola start -d` is executed, the cupola CLI shall launch the polling loop as a background daemon process detached from the controlling terminal.
3. When `cupola start -d` is executed, the cupola CLI shall record the daemon's PID to `.cupola/cupola.pid`.
4. When the daemon process starts, the cupola CLI shall redirect stdout and stderr to the log file managed by `tracing-appender`.
5. When `cupola start -d` completes successfully, the cupola CLI shall print a confirmation message including the PID to stdout before the parent process exits.
6. If `.cupola/cupola.pid` already exists and the recorded process is alive, the cupola CLI shall print an error message indicating that cupola is already running and shall exit with a non-zero status code.
7. If `.cupola/cupola.pid` already exists but the recorded process is no longer alive (stale PID), the cupola CLI shall overwrite the PID file and start the daemon normally.

---

### Requirement 3: `cupola stop` コマンドによる停止

**Objective:** cupola ユーザーとして、`cupola stop` コマンドで実行中のデーモンを確実に停止したい。これにより、スリープ復帰後やターミナルセッション終了後でも確実に cupola を停止できる。

#### Acceptance Criteria

1. The cupola CLI shall provide a `stop` subcommand.
2. When `cupola stop` is executed, the cupola CLI shall read the PID from `.cupola/cupola.pid`.
3. When `cupola stop` is executed and the process is alive, the cupola CLI shall send SIGTERM to the process identified by the PID.
4. When `cupola stop` is executed, after sending SIGTERM the cupola CLI shall wait for the process to terminate and then delete `.cupola/cupola.pid`.
5. If `.cupola/cupola.pid` does not exist, the cupola CLI shall print a message indicating that cupola is not running and shall exit with a zero status code.
6. If the process identified by the PID in `.cupola/cupola.pid` is no longer alive, the cupola CLI shall delete `.cupola/cupola.pid` and exit with a zero status code (treating it as a successful stop).
7. When `cupola stop` terminates the process successfully, the cupola CLI shall print a confirmation message to stdout.

---

### Requirement 4: PID ファイル管理

**Objective:** cupola ユーザーとして、PID ファイルが適切に管理されることで、二重起動防止と確実な停止を実現したい。

#### Acceptance Criteria

1. The cupola CLI shall write the PID file to `.cupola/cupola.pid` relative to the working directory (or the `--config`-specified cupola root).
2. The cupola CLI shall add `.cupola/cupola.pid` to `.gitignore` to prevent it from being committed to the repository.
3. While the daemon is running, the cupola CLI shall maintain the PID file at `.cupola/cupola.pid` with the correct PID.
4. When the daemon process terminates (via SIGTERM or any other signal), the cupola CLI shall delete `.cupola/cupola.pid` as part of its graceful shutdown sequence.
5. If PID file creation fails (e.g., due to permission error), the cupola CLI shall print an error message and exit with a non-zero status code.

---

### Requirement 5: グレースフルシャットダウンの保証

**Objective:** cupola ユーザーとして、`cupola stop` でデーモンが graceful shutdown されることで、処理中のタスクが安全に完了または中断されることを保証したい。

#### Acceptance Criteria

1. When the daemon process receives SIGTERM, the cupola CLI shall complete the current polling cycle before terminating.
2. When `cupola stop` is executed, if the process does not terminate within a configurable timeout (default: 30 seconds), the cupola CLI shall send SIGKILL to force termination.
3. When the daemon shuts down gracefully, the cupola CLI shall flush all pending log output before exiting.
4. The cupola CLI shall exit with status code 0 on successful graceful shutdown.
