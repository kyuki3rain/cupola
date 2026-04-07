# `cupola status`

Cupola daemon の稼働状況と、active Issue の処理状態を表示するコマンド。

## 役割

- PID ファイルから daemon 稼働状況を表示する
- DB から active Issue を読み出して一覧表示する
- 必要なら stale PID ファイルを自動で掃除する
- 設定が読める場合は `max_concurrent_sessions` を添えて現在の並行実行数を表示する

## オプション

このコマンドには専用オプションがない。

## 参照パス

現行実装では以下を固定パスで参照する。

- DB: `.cupola/cupola.db`
- config: `.cupola/cupola.toml`
- PID file: `.cupola/cupola.pid`

`--config` のような切替手段はない。

## 出力内容

先頭に daemon 状態を表示する。

- `Daemon: running (pid=...)`
- `Daemon: not running`
- `Daemon: not running (stale PID file cleaned)`
- `Daemon: not running (stale PID file exists, but cleanup failed)`

active issue がなければ次を出す。

```text
No active issues.
```

active issue がある場合は、次を表示する。

- `Running: <alive-process-count>/<max_concurrent_sessions>`
- または config が読めない場合 `Running: <alive-process-count>`
- 続いて Issue ごとの 1 行表示

## Issue 行の内容

1 行には概ね次の情報が出る。

- GitHub Issue 番号
- 現在 state
- `design_pr_number` / `impl_pr_number`
- `retry_count`
- `worktree_path`
- `error_message`
- `current_pid` と alive / dead 判定

例:

```text
  #42    DesignRunning                  design_pr:#85         retry:1 .cupola/worktrees/42 pid:12345 (alive)
```

## active Issue の定義

`status` が表示対象にするのは repository 実装の `find_active()` が返す Issue で、終端状態である `Completed` / `Cancelled` は基本的に含まれない。

状態そのものの意味は [../architecture/state-machine.md](../architecture/state-machine.md) を参照。
