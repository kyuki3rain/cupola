# Implementation Plan

- [ ] 1. panic hook の実装と組み込み
- [ ] 1.1 `install_panic_hook` 関数を実装する
  - `bootstrap/app.rs` にプライベート関数 `install_panic_hook(pid_path: PathBuf)` を追加する
  - `std::panic::take_hook()` で既存デフォルト hook を取得し move キャプチャする
  - 新しい hook クロージャ内で `tracing::error!` と `eprintln!` を使って panic 情報をログに記録する
  - `PidFileManager::new(pid_path).delete_pid()` を呼び出して PID ファイルを削除する（失敗は無視）
  - 最後にキャプチャしたデフォルト hook を呼び出す
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 1.2 `start_foreground` に `install_panic_hook` を組み込む
  - `write_pid_with_mode` の成功直後に `install_panic_hook(pid_path.clone())` を呼び出す
  - _Requirements: 1.5_

- [ ] 1.3 `start_daemon_child` に `install_panic_hook` を組み込む
  - `write_pid_with_mode` の成功直後に `install_panic_hook(pid_path.clone())` を呼び出す
  - _Requirements: 1.5_

- [ ] 2. テストの追加
- [ ] 2.1 (P) `install_panic_hook` のユニットテストを追加する
  - `bootstrap/app.rs` の `#[cfg(test)] mod tests` 内にテストを追加する
  - `install_panic_hook` 呼び出し後、`std::panic::take_hook()` で hook を取得し、偽の `PanicHookInfo` ライクなデータで直接呼び出して PID ファイルが削除されることを確認する
  - PID ファイルが存在しない場合でも hook クロージャが安全に完了することを確認する
  - _Requirements: 2.3_

- [ ] 2.2 (P) panic 時 PID ファイルクリーンアップのインテグレーションテストを追加する
  - `tests/integration_test.rs` または `tests/scenarios.rs` にテストを追加する
  - スレッドを spawn して PID ファイルを書き込み、`install_panic_hook` を設定後に `std::thread::spawn` で panic させる
  - スレッド終了後に PID ファイルが削除されていることを検証する
  - _Requirements: 2.1, 2.2_
