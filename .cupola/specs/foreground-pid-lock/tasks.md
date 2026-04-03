# 実装計画

## タスク一覧

- [ ] 1. 共有 PID チェックヘルパーの実装とリファクタリング

- [ ] 1.1 `start_daemon` の PID チェックロジックを共有ヘルパー関数として抽出する
  - `src/bootstrap/app.rs` に `fn check_and_clean_pid_file(pid_file_manager: &PidFileManager) -> Result<()>` プライベート関数を定義する
  - 既存の `start_daemon` 内インライン PID チェック（read_pid → 生存確認 → エラー or ゾンビ削除）を本関数に移動する
  - `start_daemon` からは `check_and_clean_pid_file(&pid_file_manager)?` を呼ぶよう置き換える
  - 関数の動作: `read_pid` が `Some(pid)` かつプロセス生存 → `Err("already running")`、`Some(stale)` かつ死亡 → `delete_pid` して `Ok(())`、`None` → `Ok(())`、`Err` → `Err` を伝播
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 4.2_

- [ ] 1.2 `check_and_clean_pid_file` のユニットテストを実装する
  - `check_and_clean_pid_file_returns_err_when_process_alive`: 自プロセスの PID を持つ PID ファイルを用意し、エラーが返ることを確認
  - `check_and_clean_pid_file_removes_stale_pid_and_returns_ok`: 存在しない PID を持つファイルを用意し、削除されて `Ok(())` が返ることを確認
  - `check_and_clean_pid_file_returns_ok_when_no_pid_file`: PID ファイルなしで `Ok(())` が返ることを確認
  - `check_and_clean_pid_file_returns_err_on_invalid_content`: 不正な内容の PID ファイルで `Err` が返ることを確認（`PidFileManager::read_pid` が `InvalidContent` エラーを返すケース）
  - `cargo test` で既存の `start_daemon` 関連テストが引き続き通ることを確認
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 2. foreground モードへの PID ファイル保護の追加

- [ ] 2.1 `start_foreground` に PID ファイルチェック・書き込み・クリーンアップを追加する
  - `config_dir` と PID ファイルパス (`config_dir.join("cupola.pid")`) を構築する（daemon と同一パス）
  - `PidFileManager` を構築する
  - `check_and_clean_pid_file(&pid_file_manager)?` を設定ロード直後・ロギング初期化前に呼ぶ
  - `std::process::id()` で自プロセス PID を取得し `pid_file_manager.write_pid(my_pid)` で書き込む。失敗時はエラー返却
  - `PollingUseCase::new(...)` に `.with_pid_file(Box::new(pid_file_manager))` を連結する（SIGTERM/SIGINT グレースフルシャットダウン時の削除）
  - `polling.run().await` の結果を `apply_pid_cleanup(result, pid_file_path)` でラップして返す（best-effort 終了時削除）
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 4.1, 4.3_

- [ ] 2.2 `start_foreground` の PID 保護動作を検証するテストを実装する
  - `start_foreground_check_rejects_running_pid`: 自プロセス PID を持つ PID ファイルが存在する状態でチェックがエラーを返すことをユニットテストで確認（`check_and_clean_pid_file` のテストを補強する形で可）
  - `apply_pid_cleanup` の既存テスト（4ケース）が引き続き通ることを `cargo test` で確認
  - PID ファイルパスが `<config_dir>/cupola.pid` であることを確認するテストを追加する
  - `cargo clippy -- -D warnings` と `cargo fmt --check` がエラーなしで通ることを確認
  - _Requirements: 2.3, 3.1, 3.2, 3.3, 4.3_
