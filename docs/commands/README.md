# Cupola コマンドリファレンス

`cupola` CLI が提供する各コマンドの仕様、主な振る舞い、オプション、実装上の注意点をまとめる。

## ドキュメント一覧

| ファイル | 内容 |
|---------|------|
| [start.md](./start.md) | ポーリングループの起動、foreground/background、PID 管理 |
| [stop.md](./stop.md) | PID ファイルを用いた停止処理 |
| [init.md](./init.md) | リポジトリの bootstrap と初期資産導入 |
| [doctor.md](./doctor.md) | 起動前診断と運用準備確認 |
| [status.md](./status.md) | 現在の daemon / Issue 処理状態の表示 |
| [cleanup.md](./cleanup.md) | `Cancelled` Issue の worktree / branch 整理 |
| [compress.md](./compress.md) | 完了済み spec の検出 |
| [logs.md](./logs.md) | 最新ログファイルの表示と follow |

## コマンド一覧

| コマンド | 概要 |
|---------|------|
| `cupola start` | Cupola 本体を起動して polling loop を実行する |
| `cupola stop` | PID ファイルを参照して起動中プロセスを停止する |
| `cupola init` | `.cupola/` や Claude Code 向け資産を初期化する |
| `cupola doctor` | 起動前提と運用前提を診断する |
| `cupola status` | daemon 状態と active issue 一覧を表示する |
| `cupola cleanup` | `Cancelled` 状態の Issue に紐づく worktree / branch を掃除する |
| `cupola compress` | 完了済み spec を数え、圧縮作業の実行を促す |
| `cupola logs` | 最新の `cupola.*` ログを表示する |

## 補足

- `start` だけが `docs/architecture/` で整理した polling loop を直接実行するコマンドである。
- `stop` / `status` / `logs` は主にプロセス管理・運用補助のコマンドで、polling loop の状態遷移そのものは持たない。
- `init` / `doctor` / `cleanup` / `compress` は、それぞれ独立した use case を持つ補助コマンドである。
