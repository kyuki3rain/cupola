# Implementation Plan

- [ ] 1. `Command::Cleanup` ハンドラにデーモン稼働チェックを追加する
- [ ] 1.1 PIDファイルパスを構成し `PidFileManager` をインスタンス化する
  - `config.parent().unwrap_or_else(|| Path::new(".")).join("cupola.pid")` で PID ファイルパスを構成する（`db_path` と同パターン）
  - `PidFileManager::new(pid_path)` でインスタンス化する
  - `_Requirements: 1.1, 1.2_`

- [ ] 1.2 デーモン稼働中のエラー終了ロジックを実装する
  - `pid_manager.read_pid()` を呼び出し、`Ok(Some(pid))` の場合のみ生存確認に進む
  - `pid_manager.is_process_alive(pid)` が `true` の場合、``eprintln!("Error: cupola is running (pid={pid}). Run `cupola stop` first.")`` を出力する
  - `Err(anyhow::anyhow!("daemon is running"))` を返して処理を中止する
  - `read_pid()` が `Ok(None)` または `Err(_)` の場合はチェックをスキップしてクリーンアップ処理に進む
  - 既存の警告 `println!("⚠️  daemon が動作中の場合は...")` を削除する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3, 3.1, 3.2_

- [ ] 2. デーモン稼働チェックのユニットテストを追加する
- [ ] 2.1 (P) デーモン稼働中のエラー終了をテストする
  - `read_pid()` が `Ok(Some(pid))` を返し `is_process_alive(pid)` が `true` を返すモックで、`stderr` をキャプチャしてエラー終了をアサートする
  - `stderr` が ``Error: cupola is running (pid={pid}). Run `cupola stop` first.`` と正確一致することを確認する
  - 終了ステータスが非ゼロであることを確認する
  - `CleanupUseCase` が呼び出されないことをアサートする（呼ばれた場合はパニック）
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 2.2 (P) PIDファイル不在時の正常継続をテストする
  - `read_pid()` が `Ok(None)` を返すモックで、クリーンアップ処理が継続されることをアサートする
  - _Requirements: 1.3, 3.1_

- [ ] 2.3 (P) ステールPIDファイル時の正常継続をテストする
  - `read_pid()` が `Ok(Some(pid))` を返し `is_process_alive(pid)` が `false` を返すモックで、クリーンアップ処理が継続されることをアサートする
  - _Requirements: 1.2, 3.2_

- [ ] 2.4 (P) PIDファイル読み取りエラー時の正常継続をテストする
  - `read_pid()` が `Err(_)` を返すモックで、クリーンアップ処理が継続されることをアサートする
  - _Requirements: 1.4_
