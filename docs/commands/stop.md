# `cupola stop`

PID ファイルを参照して起動中の Cupola プロセスを停止するコマンド。

## 役割

- `.cupola/cupola.pid` を読んで対象 PID を取得する
- まず `SIGTERM` を送る
- 一定時間終了を待つ
- 終了しなければ `SIGKILL` へフォールバックする
- 停止後または stale PID 判定後に PID ファイルを削除する

## PID ファイル解決

PID ファイルは `.cupola/cupola.pid`。

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

- foreground / daemon どちらの起動モードでも同じ PID ファイル規約を使うため、`stop` から停止できる
