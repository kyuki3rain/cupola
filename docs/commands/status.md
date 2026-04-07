# `cupola status`

Cupola プロセスの稼働状況と、Issue の処理状態を表示するコマンド。

## 役割

- PID ファイルからプロセス稼働状況（foreground / daemon）を表示する
- DB から Issue を読み出して一覧表示する
- 必要なら stale PID ファイルを自動で掃除する
- `max_concurrent_sessions` を添えて現在の Claude Code セッション数を表示する

## 参照パス

| ファイル | パス |
|---------|------|
| DB | `.cupola/cupola.db` |
| 設定ファイル | `.cupola/cupola.toml` |
| PID ファイル | `.cupola/cupola.pid` |

## 出力内容

先頭にプロセス状態を表示する。PID ファイルの 2 行目（`foreground` / `daemon`）を読んで起動モードを判定する。

- `Process: running (foreground, pid=...)`
- `Process: running (daemon, pid=...)`
- `Process: not running`
- `Process: not running (stale PID file cleaned)`
- `Process: not running (stale PID file exists, but cleanup failed)`

active issue がなければ次を出す。

```text
No active issues.
```

active issue がある場合は、次を表示する。

- `Claude sessions: <alive-process-count>`（`max_concurrent_sessions` が設定されている場合は `<alive>/<max>` 形式。SpawnInit による worktree 作成タスクは含まない）
- 続いて Issue ごとの 1 行表示

`--all` を指定すると `Cancelled` も含む全 Issue を表示する。`Cancelled` 行には `close_finished` の値も出す（cleanup 対象の確認や再始動不能 Issue の特定に使う）。

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
  #42    DesignRunning                  design_pr:#85         retry:1 .cupola/worktrees/issue-42 pid:12345 (alive)
```

## オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `--all` | `Cancelled` を含む全 Issue を表示する | `false` |

## active Issue の定義

デフォルトでは repository 実装の `find_active()` が返す Issue のみ表示し、終端状態である `Completed` / `Cancelled` は含まれない。`--all` 指定時は全 Issue を対象にする。

状態そのものの意味は [../architecture/state-machine.md](../architecture/state-machine.md) を参照。
