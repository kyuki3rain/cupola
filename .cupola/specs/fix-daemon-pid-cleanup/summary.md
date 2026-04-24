# fix-daemon-pid-cleanup サマリー

## Feature
daemon 正常/エラー終了時に PID ファイル削除を保証する fallback クリーンアップを `start_daemon_child` に追加。

## 要件サマリ
- `polling.run().await` が Ok/Err いずれでも PID ファイル削除を試みる。
- 削除失敗はエラー伝播させずポーリングループの結果をそのまま返す。
- 次回 `cupola start` で stale PID クリーンアップログが出ないようにする。

## アーキテクチャ決定
- オプション A（bootstrap 層のみの fallback 追加）を採用し `PollingUseCase` は変更しない。
- 既存の `with_pid_file` による graceful shutdown 経路は維持し `start_daemon_child` 側で二重保護。
- `PidFileManager::new()` は I/O を伴わないため同一パスへ 2 インスタンス作成は安全。`delete_pid()` は冪等。
- `let result = polling.run().await; let _ = cleanup_mgr.delete_pid(); result` のパターンで結果を先取りし削除失敗が `Result` を汚染しないことを保証。

## コンポーネント
- `src/bootstrap/app.rs` の `start_daemon_child` のみ
- 再利用: `PidFileManager`、`PidFilePort`

## 主要インターフェース
既存 `PidFilePort::delete_pid()` を流用。シグネチャ変更なし。

## 学び/トレードオフ
- application 層に defer/Drop ガードを入れる案は複雑化するため見送り、bootstrap 完結の最小修正で Clean Architecture 境界を維持。
- `run()` の初期化失敗など `graceful_shutdown()` を通らない経路のカバレッジが目的。既存 SIGTERM/SIGINT パスには影響なし。
