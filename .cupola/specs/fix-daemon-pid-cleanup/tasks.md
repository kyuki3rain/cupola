# Implementation Plan

- [x] 1. start_daemon_child に fallback PID ファイル削除を追加する
  - `polling.run().await` の戻り値を変数に保存し、その後 cleanup 専用の `PidFileManager` インスタンスで `delete_pid()` を呼ぶ
  - `delete_pid()` の結果は `let _ = ...` で無視し、ポーリングループの結果のみを返す
  - cleanup 用 `PidFileManager` のパスは既存の `config_dir.join("cupola.pid")` と同一にする
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 2. 正常終了パスと fallback 削除の単体テストを追加する
- [x] 2.1 ポーリングループ終了後の PID ファイル削除を検証する
  - `polling.run()` が `Ok(())` を返した場合に PID ファイルが削除されることを確認する
  - `polling.run()` が `Err(e)` を返した場合でも PID ファイルが削除されることを確認する
  - PID ファイル削除が失敗しても `polling.run()` の `Result` がそのまま返されることを確認する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 2.2 (P) 冪等性と次回起動時の挙動を検証する
  - PID ファイルが既に存在しない状態で `delete_pid()` を呼んでも `Ok(())` が返されることを確認する
  - 正常終了後の次回起動で `read_pid()` が `Ok(None)` を返すことを確認する
  - _Requirements: 1.3, 2.1, 2.2_
