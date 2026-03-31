# Implementation Plan

## 実装タスク一覧

- [ ] 1. `nix` クレートの依存追加
- [ ] 1.1 `Cargo.toml` に `nix` クレートを Unix 限定依存として追加する
  - `[target.'cfg(unix)'.dependencies]` セクションに `nix = { version = "0.29", features = ["process", "signal", "unistd"] }` を追加する
  - `cargo build` でコンパイルエラーが出ないことを確認する
  - _Requirements: 2.2, 3.3_

- [ ] 2. PID ファイル操作ポートの定義
- [ ] 2.1 (P) `PidFilePort` トレイトと `PidFileError` エラー型を application/port に定義する
  - PID の書き込み・読み取り・削除・プロセス生死確認の 4 メソッドを持つトレイトを定義する
  - `PidFileError` は `thiserror` を使い Write/Read/Delete/InvalidContent の 4 バリアントを持つ
  - `port/mod.rs` への re-export を追加する
  - _Requirements: 4.1, 4.3, 4.5_

- [ ] 3. CLI サブコマンドの更新
- [ ] 3.1 (P) `Command::Run` を `Command::Start` にリネームし `-d/--daemon` フラグを追加する
  - `Command::Run` 列挙子を `Command::Start` に名称変更し、`daemon: bool` フィールドを追加する（`-d` / `--daemon`）
  - 既存のオプション `--polling-interval-secs`, `--log-level`, `--config` はそのまま継承する
  - `Command::Stop { config: PathBuf }` 列挙子を新規追加する
  - cli.rs 内のテストを `Command::Start` に対応させ、`-d` フラグのパーステストを追加する
  - _Requirements: 1.1, 1.2, 1.4, 2.1, 3.1_

- [ ] 4. PidFileManager の実装
- [ ] 4.1 `PidFileManager` を adapter/outbound に実装し `PidFilePort` を実装する
  - `pid_file_path: PathBuf` を持つ `PidFileManager` 構造体を実装する（タスク 1 完了後に着手）
  - `write_pid`: ファイルに `"{pid}\n"` を書き込む
  - `read_pid`: ファイルを読み `u32` にパース。ファイルなしは `Ok(None)`、不正内容は `InvalidContent` エラー
  - `delete_pid`: ファイルが存在しない場合も `Ok(())` を返す
  - `is_process_alive`: `nix::sys::signal::kill(Pid::from_raw(pid as i32), None)` で確認。`EPERM` はプロセス生存として扱う
  - _Requirements: 2.3, 3.2, 4.1, 4.3, 4.4, 4.5_

- [ ] 4.2 `PidFileManager` のユニットテストを実装する
  - `tempfile::NamedTempFile` を使い `write_pid` → `read_pid` のラウンドトリップを検証する
  - `delete_pid` でファイルが削除されること・ファイルなし時も Ok になることを確認する
  - `is_process_alive` で `std::process::id()` (自プロセス) が alive、不正 PID（u32::MAX）が dead になることを確認する
  - _Requirements: 4.1, 4.3, 4.4, 4.5_

- [ ] 5. StopUseCase の実装
- [ ] 5.1 (P) `StopUseCase` を application 層に実装する（タスク 2 完了後に着手）
  - `PidFilePort` を保持し `shutdown_timeout: Duration` を設定可能にする
  - `execute()` はタスクフローに従い: PID 読み取り → プロセス生死確認 → SIGTERM 送信 → ポーリング待機 → タイムアウト時 SIGKILL → PID 削除 の順で処理する
  - `StopResult` enum（Stopped / StalePidCleaned / NotRunning / ForceKilled）を定義する
  - `StopError` enum（PidFile / Signal）を `thiserror` で定義する
  - SIGTERM 送信には `nix::sys::signal::kill(pid, Signal::SIGTERM)` を使用する
  - 500ms 間隔で `is_process_alive` をポーリングし、タイムアウト（デフォルト 30 秒）超過で SIGKILL を送信する
  - _Requirements: 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 5.2, 5.4_

- [ ] 5.2 `StopUseCase` のユニットテストを実装する
  - モック `PidFilePort` を用意し、PID ファイルなし → `NotRunning` を返すことを確認する
  - ステール PID（プロセス死亡済み）のケースで `StalePidCleaned` を返すことを確認する
  - _Requirements: 3.5, 3.6_

- [ ] 6. PollingUseCase への SIGTERM 対応と PID ファイルクリーンアップ統合
- [ ] 6.1 (P) `PollingUseCase` に SIGTERM ハンドリングを追加する（タスク 2 完了後に着手）
  - `polling_use_case.rs` の `run()` メソッドの `tokio::select!` に `tokio::signal::unix::signal(SignalKind::terminate())` ブランチを追加する
  - SIGTERM 受信時に `"received SIGTERM, shutting down..."` をログ出力してループを抜ける
  - `PollingUseCase` に `pid_file: Option<Box<dyn PidFilePort>>` フィールドを追加する（フォアグラウンド起動時は `None`）
  - `graceful_shutdown()` の末尾で `pid_file` が `Some` の場合に `delete_pid()` を呼ぶ
  - _Requirements: 1.5, 4.4, 5.1, 5.3, 5.4_

- [ ] 7. InitFileGenerator の .gitignore 更新
- [ ] 7.1 (P) `GITIGNORE_ENTRIES` 定数に `.cupola/cupola.pid` を追加する
  - `src/adapter/outbound/init_file_generator.rs` の `GITIGNORE_ENTRIES` 定数に `.cupola/cupola.pid` を追記する
  - 既存の `.gitignore` テストが引き続きパスすることを確認する
  - _Requirements: 4.2_

- [ ] 8. app.rs のコマンドディスパッチとデーモン化ロジックの実装
- [ ] 8.1 `Command::Start` のディスパッチを app.rs に実装する
  - `Command::Run` のマッチアームを `Command::Start { daemon, ... }` に対応させる
  - `daemon: false` の場合は既存の `run` フローと完全に同一の動作を維持する（`pid_file: None` で `PollingUseCase` を構築）
  - `daemon: true` の場合は `start_daemon()` ヘルパー関数に処理を委譲する
  - _Requirements: 1.3, 1.4_

- [ ] 8.2 デーモン化ロジック（`start_daemon` 関数）を実装する
  - 設定の `log_dir` が未設定の場合はエラーを返す（ログ先不明でのデーモン起動禁止）
  - `PidFileManager::new(config_dir.join("cupola.pid"))` で PID ファイルパスを構築する
  - `read_pid()` → `is_process_alive()` で二重起動確認。生存中はエラーメッセージを表示して非ゼロ終了する
  - ステール PID が検出された場合は上書きせず削除して続行する
  - `nix::unistd::fork()` で子プロセスを生成する
  - 親プロセスは `child_pid` を `println!("started cupola daemon (pid={})", child_pid)` して `std::process::exit(0)` する
  - 子プロセスは `nix::unistd::setsid()` → stdin を `/dev/null` にリダイレクト → `write_pid(getpid())` → ポーリングループ起動の順で処理する
  - `PollingUseCase` は `pid_file: Some(Box::new(pid_file_manager))` で構築する
  - _Requirements: 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 4.1, 4.5_

- [ ] 8.3 `Command::Stop` のディスパッチを app.rs に実装する
  - `StopUseCase::new(pid_file_manager, Duration::from_secs(30))` を構築する
  - `execute()` の結果に応じてユーザーメッセージを stdout に出力する
    - `NotRunning` → `"cupola is not running"`
    - `StalePidCleaned` → `"cleaned up stale PID file"`
    - `Stopped { pid }` → `"stopped cupola (pid={pid})"`
    - `ForceKilled { pid }` → `"force killed cupola (pid={pid})"`
  - 失敗時は `Err` を返しプロセスを非ゼロ終了させる
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

- [ ] 9. 統合テストとエンドツーエンド検証
- [ ] 9.1 フォアグラウンド起動のリグレッションテストを確認する
  - `cupola start`（`daemon: false`）が従来の `cupola run` と同等に動作することを既存テストで検証する
  - app.rs の `status_with_no_active_issues` / `status_with_active_issues` テストが引き続きパスすることを確認する
  - _Requirements: 1.3_

- [ ] 9.2 (P) SIGTERM によるポーリングループ停止の統合テストを実装する
  - `tokio::spawn` でポーリングループを起動し、`nix::sys::signal::kill(std::process::id(), SIGTERM)` を送信して正常終了することを検証する
  - `graceful_shutdown()` が呼ばれ、モック `PidFilePort` の `delete_pid()` が呼ばれることを確認する
  - _Requirements: 1.5, 4.4, 5.1_

- [ ] 9.3 (P) `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test` を通してコード品質を確認する
  - `cargo fmt` でフォーマットの問題がないことを確認する
  - `cargo clippy -- -D warnings` で警告がゼロであることを確認する
  - `cargo test` で全テストがパスすることを確認する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3, 5.4_
