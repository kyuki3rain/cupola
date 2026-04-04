# 実装タスク: Clean Architecture 違反修正 + anyhow エラー統一

## タスク一覧

- [ ] 1. 未使用コードの削除
- [ ] 1.1 `CupolaError`（`application/error.rs`）を削除し、参照を全て除去する
  - `src/application/error.rs` ファイルを削除する
  - コードベース全体で `CupolaError` をインポートまたは参照している箇所を検索し、除去する
  - 削除後に `cargo build` が警告なしで成功することを確認する
  - _Requirements: 1.1, 1.3, 1.4_

- [ ] 1.2 `domain/check_result.rs`（デッドコード）を削除し、参照を全て除去する
  - `src/domain/check_result.rs` ファイルを削除する
  - `mod.rs` 等から `check_result` モジュール宣言を削除する
  - 削除後に `cargo build` が警告なしで成功することを確認する
  - _Requirements: 1.2, 1.3, 1.4_

- [ ] 2. `SignalPort` の分離と `NixSignalSender` の adapter 層への移動
- [ ] 2.1 (P) `SignalPort` トレイトを `application/port/signal.rs` に移動する
  - `stop_use_case.rs` 内の `SignalPort` トレイト定義を新規ファイル `application/port/signal.rs` に移動する
  - `application/port/mod.rs` に `signal` モジュールを追加する
  - `stop_use_case.rs` のインポートを `crate::application::port::signal::SignalPort` に更新する
  - _Requirements: 3.1_

- [ ] 2.2 (P) `NixSignalSender` を `adapter/outbound/nix_signal_sender.rs` に移動する
  - `stop_use_case.rs` 内の `NixSignalSender` 構造体および `SignalPort` 実装を `adapter/outbound/nix_signal_sender.rs` に移動する
  - `adapter/outbound/mod.rs` に `nix_signal_sender` モジュールを追加する
  - 移動後、`stop_use_case.rs` から `nix` クレートのインポートを削除する
  - _Requirements: 3.1, 3.2_

- [ ] 2.3 `StopUseCase` の import パスを修正し、bootstrap の DI を `with_signal_sender` に変更する
  - タスク 2.1 完了後: `stop_use_case.rs` の `SignalPort` インポートパスが正しいことを確認する
  - `StopUseCase::new(pid_file, Duration)` を使用している bootstrap コードを `StopUseCase::with_signal_sender(pid_file, NixSignalSender, Duration)` に変更する
  - `stop_use_case.rs` 内の `impl<P> StopUseCase<P, NixSignalSender>` ブロック（`NixSignalSender` を内部生成するコンストラクタ）を削除するか bootstrap 専用に制限する
  - 既存のユニットテスト（`MockSignal` を使うテスト群）が引き続きパスすることを確認する
  - _Requirements: 3.3, 3.4_

- [ ] 3. (P) `DoctorUseCase` への `CommandRunner` ポート注入
- [ ] 3.1 `DoctorUseCase` 構造体に `CommandRunner` フィールドを追加し、コマンド呼び出しをポート経由にする
  - `DoctorUseCase<C: ConfigLoader>` を `DoctorUseCase<C: ConfigLoader, R: CommandRunner>` に変更する
  - `check_git`・`detect_gh_presence`・`check_gh_label`・`check_weight_labels` の各関数が `&dyn CommandRunner`（または `&R`）を引数として受け取るよう変更する
  - 各関数内の `std::process::Command::new(...)` を `command_runner.run(program, args)` に置き換える
  - `CommandOutput` の `success` / `stdout` / `stderr` フィールドを使ってエラー判定を行う
  - _Requirements: 4.1, 4.2, 4.3_

- [ ] 3.2 bootstrap での注入を追加し、既存テストを `MockCommandRunner` 経由に更新する
  - bootstrap の `doctor` コマンドハンドラで `ProcessCommandRunner::new()` を生成して `DoctorUseCase::new(loader, runner)` に注入する
  - `doctor_use_case.rs` 内の `DoctorUseCase::new(loader)` を使った全テストを `DoctorUseCase::new(loader, mock_runner)` 形式に更新する
  - `MockCommandRunner` を使って各チェック（git 存在、gh 認証、label 存在）の成功・失敗パターンをユニットテストで検証する
  - _Requirements: 4.4, 4.5_

- [ ] 4. `InitUseCase` のポート依存化
- [ ] 4.1 (P) `DbInitializer` ポートを定義し、`SqliteConnection` に実装させる
  - `application/port/db_initializer.rs` に `DbInitializer` トレイト（`init_schema(&self) -> anyhow::Result<()>`）を定義する
  - `application/port/mod.rs` に `db_initializer` モジュールを追加する
  - `adapter/outbound/sqlite_connection.rs` で `SqliteConnection` に `DbInitializer` を実装する
  - _Requirements: 2.1, 2.6_

- [ ] 4.2 (P) `FileGenerator` ポートを定義し、`InitFileGenerator` に実装させる
  - `application/port/file_generator.rs` に `FileGenerator` トレイト（3 メソッド: `generate_toml_template`, `copy_steering_templates`, `append_gitignore_entries`、いずれも `anyhow::Result<bool>` を返す）を定義する
  - `application/port/mod.rs` に `file_generator` モジュールを追加する
  - `adapter/outbound/init_file_generator.rs` で `InitFileGenerator` に `FileGenerator` を実装する
  - _Requirements: 2.2, 2.6_

- [ ] 4.3 `InitUseCase` のシグネチャを変更し、bootstrap DI とテストを更新する
  - タスク 4.1・4.2 完了後に実施する
  - `InitUseCase::new(base_dir)` を `InitUseCase::new(base_dir, db_init, file_gen)` に変更する
  - `init_use_case.rs` から `crate::adapter::outbound::*` のインポートを全て削除する
  - bootstrap の `init` コマンドハンドラで `SqliteConnection` と `InitFileGenerator` を生成して `InitUseCase::new` に渡すよう変更する
  - `init_use_case.rs` の既存テストを `InitUseCase::new(base_dir, db, file_gen)` シグネチャに合わせて更新する（具象型を使う統合テストとして維持）
  - _Requirements: 2.3, 2.4, 2.5_

- [ ] 5. エラー型統一の確認と全体ビルド・テスト検証
- [ ] 5.1 use case 戻り値型の `anyhow::Result` 統一を確認し、不整合を修正する
  - 全 use case（`init_use_case`, `stop_use_case`, `doctor_use_case`, `polling_use_case`, `transition_use_case` 等）の公開メソッドが `anyhow::Result<T>` を使用していることを確認する
  - adapter 層の各アウトバウンドアダプタが外部ライブラリエラーを `.context()` または `.with_context()` でラップして返していることを確認する
  - application 層の port トレイトメソッドが `anyhow::Result<T>` を返しているか確認する（ポート固有エラー型 `StopError`・`PidFileError`・`ConfigLoadError` はそのまま維持）
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [ ] 5.2 `cargo build`、`cargo test`、`cargo clippy -- -D warnings` を全パスさせる
  - `cargo build` を実行し、コンパイルエラー・警告を全て修正する
  - `cargo clippy -- -D warnings` を実行し、全ての lint 警告を修正する
  - `cargo test` を実行し、全テストがパスすることを確認する
  - タスク 1〜5.1 で修正したファイルに clippy 指摘がある場合は修正する
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_
