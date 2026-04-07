# Cupola コマンドリファレンス

`cupola` CLI が提供する各コマンドの仕様、主な振る舞い、オプション、実装上の注意点をまとめる。

## ドキュメント一覧

| ファイル | 内容 |
|---------|------|
| [start.md](./start.md) | ポーリングループの起動、foreground/background、PID 管理 |
| [stop.md](./stop.md) | PID ファイルを用いた停止処理 |
| [init.md](./init.md) | リポジトリの bootstrap と初期資産導入 |
| [doctor.md](./doctor.md) | 起動前診断と運用準備確認 |
| [status.md](./status.md) | 現在のプロセス / Issue 処理状態の表示 |
| [cleanup.md](./cleanup.md) | `Cancelled` Issue の worktree / branch / PR の完全リセット |
| [compress.md](./compress.md) | 完了済み spec の検出と圧縮 |
| [logs.md](./logs.md) | 最新ログファイルの表示と follow |

## コマンド一覧

| コマンド | 概要 |
|---------|------|
| `cupola start` | Cupola 本体を起動して polling loop を実行する |
| `cupola stop` | PID ファイルを参照して起動中プロセスを停止する |
| `cupola init` | `.cupola/` ディレクトリと Claude Code 向け資産を初期化する |
| `cupola doctor` | 起動前提と運用前提を診断する |
| `cupola status` | プロセス状態と active issue 一覧を表示する |
| `cupola cleanup` | `Cancelled` 状態の Issue を完全リセットする（worktree / branch / PR） |
| `cupola compress` | 完了済み spec を数え、Claude Code で圧縮する |
| `cupola logs` | 最新の `cupola.*` ログを表示する |

## ディレクトリ構造

すべてのファイルは `.cupola/` 配下（またはリポジトリ直下の `.claude/`）に収まる。

```
.cupola/
  cupola.toml        ← committed
  specs/             ← committed
  steering/          ← committed
  settings/          ← committed
  .gitignore         ← committed（runtime ファイルを無視）
  cupola.db          ← ignored
  cupola.pid         ← ignored
  logs/              ← ignored
  worktrees/         ← ignored
  inputs/            ← ignored

.claude/commands/cupola/   ← committed（init が生成する静的ファイル）
```

## 使用シナリオ

### セットアップから起動・停止まで

新しいリポジトリで Cupola を使い始めるときの一連の流れ。

```bash
# 1. 初期化（.cupola/ ディレクトリと各種資産を生成）
$ cupola init
  database:   initialized
  config:     created (.cupola/cupola.toml)
  .gitignore: created (.cupola/.gitignore)
  assets:     installed (claude-code)
  steering:   ok
```

`cupola.toml` を開いて `owner` / `repo` / `default_branch` / `language` を設定する。

```bash
# 2. 起動前診断（全項目 OK を確認）
$ cupola doctor
Start Readiness
  ✅ config        .cupola/cupola.toml loaded
  ✅ git           git version 2.47.0
  ✅ github token  authenticated
  ✅ claude CLI    claude 1.2.3
  ✅ database      .cupola/cupola.db exists

Operational Readiness
  ✅ assets        commands and settings found
  ✅ steering      3 file(s) found
  ✅ agent:ready label    found
  ✅ weight labels        light / medium / heavy found
```

```bash
# 3. バックグラウンドで起動
$ cupola start --daemon
started cupola (pid=12345)
```

```bash
# 4. GitHub Issue に agent:ready ラベルを付けてしばらく待つ
#    処理が始まったら状態を確認
$ cupola status
Process: running (daemon, pid=12345)
Claude sessions: 1/3

  #42    DesignRunning     design_pr:-            retry:0  .cupola/worktrees/issue-42  pid:23456 (alive)
```

```bash
# 5. ログをリアルタイムで確認したいとき
$ cupola logs -f
2026-04-07T10:23:01  INFO polling cycle start  issue=42 state=DesignRunning
...
```

```bash
# 6. 停止
$ cupola stop
stopped cupola (pid=12345)
```

---

### 完了済み spec を圧縮する

Issue がいくつか `Completed` になると `.cupola/specs/` に済み spec が溜まる。spec 1件につき requirements.md / design.md / tasks.md 等が生成されるため、放置するとディレクトリ全体の分量が大きくなる。定期的に圧縮して spec を軽量なサマリに置き換える。

```bash
# 完了済み spec を検出して Claude Code を自動起動
$ cupola compress
3 件の完了済み spec が見つかりました。
# → Claude Code が起動し /cupola:spec-compress を実行
#   各 spec が要約されて archived フェーズに移行する
```

目安として **数件溜まったタイミング**、または **長期間 Cupola を動かし続けた後**に実行する。`claude` CLI が導入されていない場合はスキップされる。

---

### Cupola をアップグレードする

バイナリを新しいバージョンに入れ替えた後、資産ファイルを更新する流れ。

```bash
# 1. doctor でバージョンを確認
$ cupola doctor
Operational Readiness
  ⚠️ assets version  v1.0.0 → v1.2.0 available
     fix: cupola init --upgrade
```

```bash
# 2. 資産ファイルを最新版で上書き
$ cupola init --upgrade
  settings/rules/*:             updated
  settings/templates/*:         updated
  .claude/commands/cupola/*:    updated
  .gitignore:                   updated
  cupola.toml:                  skipped (user-owned)
  steering/*:                   skipped (user-owned)
```

```bash
# 3. 再確認
$ cupola doctor
Operational Readiness
  ✅ assets version  v1.2.0
```

`settings/` や `.claude/commands/cupola/` をカスタマイズしている場合は、`--upgrade` 前にバックアップを取るか、アップグレード後に差分を確認して手動で補正すること。

---

### Cancelled Issue のリソースを掃除する

retry 上限に達した Issue は `Cancelled` 状態のまま worktree とブランチが残る。ディスクや Git リモートを整理したいときに `cleanup` を使う。

```bash
# まず停止してから実行する
$ cupola stop
stopped cupola (pid=12345)

$ cupola cleanup
対象: 2 件の Cancelled Issue

  #38  worktree: removed  branches: removed  prs: closed  metadata: cleared
  #41  worktree: not found (already deleted)  branches: removed  prs: none  metadata: cleared

2 件処理しました。
```

`Cancelled` になった原因（CI 環境の問題、認証切れ等）を修正してから再び Issue を reopen すれば、Cupola が次サイクルで再始動する。retry 枯渇で Cancelled になった場合も、`Cancelled → Idle` 遷移時に失敗カウントがリセットされるため同様に再始動できる。cleanup した場合は worktree・branch の削除と open PR の close が行われるため、再始動後は必ず InitializeRunning から再実行される。

---

## 補足

- `start` だけが `docs/architecture/` で整理した polling loop を直接実行するコマンドである。
- `stop` / `status` / `logs` は主にプロセス管理・運用補助のコマンドで、polling loop の状態遷移そのものは持たない。
- `init` / `doctor` / `cleanup` / `compress` は、それぞれ独立した use case を持つ補助コマンドである。
