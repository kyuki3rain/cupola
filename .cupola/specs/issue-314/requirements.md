# 要件定義書

## はじめに

本ドキュメントは Issue #314「panic 時に子プロセスを回収する graceful exit (panic hook)」の要件を定義する。

デーモン内で panic が発生した際、子プロセス（Claude Code）が孤児化する問題を **panic hook ベースの graceful exit** で解決する。現在の `install_panic_hook` は PID ファイル削除とログ記録を行っているが、子プロセスのシャットダウンが欠落している。本機能では以下を保証する：

1. 子プロセス PID を panic hook から安全に参照できる共有レジストリを導入する
2. panic hook 内で子プロセスへ SIGTERM → タイムアウト後 SIGKILL を送る同期シャットダウンを実行する
3. panic の発生場所とメッセージを確実にログに残す
4. PID ファイルを削除してデーモンの再起動を妨げない

Issue #309 (Mutex 毒化対策) を本 issue に統合してクローズする。

## 要件

### 要件 1: 子プロセス PID 共有レジストリ

**目的**: As a cupola デーモン, I want SessionManager と panic hook が同じ子プロセス PID セットを参照できること, so that panic が起きても登録済みの全子プロセスに確実にシャットダウン信号を送れる。

#### 受け入れ条件

1. The system shall maintain a shared registry of active child process PIDs that is accessible from both the async polling loop and the synchronous panic hook.
2. When a child process is registered via `SessionManager::register`, the system shall add the child's PID to the shared registry.
3. When a child process exits (detected by `collect_exited`) or is killed (via `kill` or `kill_all`), the system shall remove the child's PID from the shared registry.
4. The system shall implement the registry using `Arc<Mutex<HashSet<u32>>>` to ensure safe concurrent access across thread boundaries.
5. If `SessionManager` is configured without a registry, the system shall operate as before (registry integration is opt-in via builder pattern).

### 要件 2: 同期シャットダウン API

**目的**: As a cupola デーモン, I want panic hook（同期コンテキスト）から子プロセスを確実に終了させるための API があること, so that tokio runtime が停止していても全子プロセスを回収できる。

#### 受け入れ条件

1. When the sync shutdown API is invoked, the system shall send SIGTERM to all PIDs in the shared registry.
2. When SIGTERM is sent, the system shall poll each process's liveness (via `kill(pid, 0)`) at short intervals until all processes exit or the SIGTERM timeout (3 秒) elapses.
3. If any child process has not exited after the SIGTERM timeout, the system shall send SIGKILL to that process.
4. The system shall perform the entire shutdown sequence without requiring an async/await context (sync-only API).
5. If the registry Mutex is poisoned at the time of the call, the system shall recover the inner value via `PoisonError::into_inner` and proceed with shutdown rather than aborting.

### 要件 3: panic hook の拡張

**目的**: As a cupola オペレーター, I want デーモンが panic した際に子プロセスの自動回収・PID ファイル削除・ログ記録がすべて実行されること, so that ユーザーが手動で孤児プロセスをクリーンアップしなくて済む。

#### 受け入れ条件

1. When the daemon panics, the system shall invoke the sync shutdown API to terminate all registered child processes before any other cleanup.
2. When the daemon panics, the system shall delete the PID file (best-effort; errors are silently ignored).
3. When the daemon panics, the system shall log the panic message and file/line location via `tracing::error!`.
4. When all cleanup is complete, the system shall invoke the previously installed hook (default hook) to preserve standard panic output (stderr backtrace / abort).
5. The system shall perform child process shutdown before PID file deletion, ensuring the shutdown sequence is: child termination → PID deletion → default hook.

### 要件 4: bootstrap 配線

**目的**: As a cupola 開発者, I want bootstrap 起動時に共有レジストリが正しく panic hook と SessionManager の両方に接続されること, so that 実際の panic 経路で全コンポーネントが協調して動作する。

#### 受け入れ条件

1. When starting in foreground or daemon-child mode, the system shall create the `ChildProcessRegistry` before writing the PID file and installing the panic hook.
2. The system shall pass one clone of the registry to `install_panic_hook` and another clone to the `PollingUseCase` (which forwards it to `SessionManager`).
3. `PollingUseCase` shall expose a `with_child_registry` builder method (analogous to `with_process_repo` and `with_pid_file`) that wires the registry into `SessionManager`.
4. The system shall install the extended panic hook (with child process shutdown) in both `start_foreground` and `start_daemon_child`.

### 要件 5: 統合テスト

**目的**: As a cupola 開発者, I want panic シナリオで孤児プロセスが残らないことを自動テストで検証できること, so that 将来の変更でリグレッションが発生しても即座に検出できる。

#### 受け入れ条件

1. When a Mutex poisoning panic is triggered (simulating `SqliteConnection::conn_lock` poisoning), the integration test shall verify that no child process PIDs from the registry remain alive after the hook executes.
2. When a `spawn_blocking` panic is triggered, the integration test shall verify that no registered child process PIDs remain alive after the hook executes.
3. After the panic hook executes, the integration test shall verify that the PID file has been deleted.
4. The integration test shall complete within a reasonable time bound (SIGTERM timeout + margin) to avoid flaky CI runs.
5. Where the existing `kill_all` behavior is tested, the system shall add regression tests confirming that registry entries are properly removed after `kill` and `kill_all` operations.
