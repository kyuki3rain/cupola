# Implementation Plan

- [ ] 1. ドメイン層: ShutdownMode と Config の拡張
- [ ] 1.1 (P) ShutdownMode enum を domain 層に追加する
  - `None` / `Graceful { deadline: Option<Instant> }` / `Force` の3状態を定義する
  - ドメイン純粋型として I/O 依存なし
  - _Requirements: 1.1, 1.2, 2.2, 2.3, 5.4_

- [ ] 1.2 (P) Config に shutdown_timeout フィールドを追加する
  - `pub shutdown_timeout: Option<Duration>` フィールドを `src/domain/config.rs` に追加する
  - `None` = 無限待機、`Some(t)` = 指定秒タイムアウトのセマンティクスをドキュメント化する
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [ ] 2. Bootstrap 層: 設定読み込みの拡張
- [ ] 2.1 CupolaToml に shutdown_timeout_secs を追加し Config への変換ロジックを実装する
  - `CupolaToml` に `pub shutdown_timeout_secs: Option<u64>` を追加する
  - 変換: `None` → `Some(300s)`、`Some(0)` → `None`、`Some(n)` → `Some(ns)`
  - ユニットテストで3ケース（未設定・0・正数）を検証する
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 3. Application 層: StopUseCase の force フラグ対応
- [ ] 3.1 StopUseCase のコンストラクタとシグネチャを拡張する
  - `new` の `shutdown_timeout` 引数を `Option<Duration>` に変更する
  - `execute(&self, force: bool)` シグネチャに `force` フラグを追加する
  - `force = true` の場合 SIGTERM をスキップして SIGKILL を即送信するブランチを追加する
  - _Requirements: 2.1, 2.3, 4.2, 4.3, 4.4_

- [ ] 3.2 StopUseCase のユニットテストを更新・追加する
  - 既存テストの `execute()` 呼び出しを `execute(false)` に更新する
  - `execute(true)` で SIGTERM が呼ばれず SIGKILL が呼ばれることをテストする
  - `shutdown_timeout = None` 時にタイムアウトループが無限ループ（または呼び出し元が中断）となることを確認する
  - _Requirements: 2.1, 4.3, 5.1_

- [ ] 4. Application 層: PollingUseCase の graceful shutdown 改修
- [ ] 4.1 PollingUseCase コンストラクタに shutdown_timeout を追加し run ループを改修する
  - `shutdown_timeout: Option<Duration>` をコンストラクタ引数に追加する
  - SIGINT 複数回受信のため `tokio::signal::unix::signal(SignalKind::interrupt())` に移行する
  - シグナル受信時に `ShutdownMode` を設定する分岐ロジックを実装する（SIGTERM → Graceful、2回目 SIGINT → Force）
  - 二重 SIGTERM 受信時にログを出力してタイマーリセットなし（要件 5.4）
  - _Requirements: 1.1, 1.2, 2.2, 5.4_

- [ ] 4.2 graceful_shutdown() を ShutdownMode に対応させる
  - `ShutdownMode::Graceful` の場合: 100ms ポーリングで全セッション完了を待ち、`deadline` 超過時に `kill_all()` → タイムアウトログ
  - `ShutdownMode::Force` の場合: 即 `session_mgr.kill_all()` → 最大 5 秒回収待ち
  - 5 秒ごとに残セッション数ログを出力する（要件 1.5）
  - 全セッション完了時に完了数ログを出力する（要件 5.3）
  - タイムアウト強制終了時に強制終了ログを出力する（要件 5.2）
  - _Requirements: 1.3, 1.4, 1.5, 2.3, 5.2, 5.3_

- [ ] 4.3 PollingUseCase の統合テストを追加する
  - SIGTERM 受信後に新規セッション起動が停止することを mock で確認する
  - graceful_shutdown でタイムアウト後に `kill_all` が呼ばれることを確認する
  - 2 回目 SIGINT 受信後に即 `kill_all` が呼ばれることを確認する
  - _Requirements: 1.1, 1.2, 1.3, 2.2, 2.3_

- [ ] 5. Adapter 層: CLI `--force` フラグ追加
- [ ] 5.1 clap derive の `Commands::Stop` に `--force` フラグを追加する
  - `force: bool` フィールドを `Commands::Stop` に追加する
  - bootstrap の stop ハンドラで `force` 値を `StopUseCase::execute(force)` に渡す
  - bootstrap で `StopUseCase` 生成時に `Config.shutdown_timeout` を渡す
  - _Requirements: 4.1, 4.2, 4.3, 2.1_

- [ ] 6. Bootstrap 層: PollingUseCase への shutdown_timeout 注入
- [ ] 6.1 PollingUseCase 生成時に Config.shutdown_timeout を渡す
  - `app.rs` の foreground / daemon 両モードで `PollingUseCase::new` の引数に `shutdown_timeout` を追加する
  - StopUseCase 生成時のハードコード `Duration::from_secs(30)` を `Config.shutdown_timeout` に置き換える
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_
