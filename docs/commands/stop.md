# `cupola stop`

PID ファイルを参照して起動中の Cupola プロセスを停止するコマンド。

## 役割

- `cupola.pid` を読んで対象 PID を取得する
- まず `SIGTERM` を送る
- 一定時間終了を待つ
- 終了しなければ `SIGKILL` へフォールバックする
- 停止後または stale PID 判定後に PID ファイルを削除する

## オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `--config <path>` | 設定ファイルのパス。PID ファイル位置の基準にも使う | `.cupola/cupola.toml` |

## PID ファイル解決

- PID ファイルは `--config` の親ディレクトリ配下の `cupola.pid`
- 例: `--config .cupola/cupola.toml` なら `.cupola/cupola.pid`

## 停止手順

1. PID ファイルを読む
2. PID がなければ `NotRunning`
3. PID が不正なら削除してエラー
4. プロセスが既に死んでいれば stale PID とみなして削除
5. `SIGTERM` を送る
6. 500ms 間隔で終了確認する
7. 30 秒を超えたら `SIGKILL` を送る
8. 停止確認後に PID ファイルを削除する

## 出力

標準出力には次のいずれかを出す。

- `cupola is not running`
- `cleaned up stale PID file`
- `stopped cupola (pid=...)`
- `force killed cupola (pid=...)`

## 補足

- help 文言は「background daemon を止める」だが、実装上は PID ファイルに登録されている Cupola プロセス全般を対象にする
- `start` の foreground 実行も同じ PID ファイル規約を使うため、同一設定パスで起動していれば `stop` から停止できる
