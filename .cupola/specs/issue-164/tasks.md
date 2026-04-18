# Implementation Plan

- [ ] 1. ドメイン層: ShutdownMode と Config の拡張
- [ ] 1.1 (P) ShutdownMode enum を domain 層に追加する
  - 通常動作・graceful 待機（タイムアウト付き/なし）・強制終了の3状態を型安全に表現できるようにする
  - ドメイン純粋型として定義し、I/O 依存を持たない
  - _Requirements: 1.1, 1.2, 2.2, 2.3, 5.4_

- [ ] 1.2 (P) Config に shutdown_timeout フィールドを追加する
  - ドメイン設定として shutdown タイムアウト値を保持できるようにする（無限待機と秒数タイムアウトの両方に対応）
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [ ] 2. Bootstrap 層: 設定読み込みの拡張
- [ ] 2.1 CupolaToml に shutdown_timeout_secs を追加し Config への変換ロジックを実装する
  - toml 設定ファイルから shutdown タイムアウト値を読み込み、ドメイン表現へ変換する
  - 変換: 未設定 → デフォルト 300 秒、0 → 無限待機、正の整数 → n 秒タイムアウト
  - ユニットテストで3ケース（未設定・0・正数）を検証する
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 3. Application 層: StopUseCase の force フラグ対応
- [ ] 3.1 StopUseCase に force フラグと進捗表示機能を追加する
  - force フラグに応じて graceful 停止と強制停止を切り替えられるようにする
  - force 停止時: SIGKILL 送信前にセッション状態ファイルからアクティブセッション数を読み取り、終了セッション数として結果に含める（AC 2.4）
  - graceful 停止時: デーモン死活確認ループ内でセッション状態ファイルをポーリングし、残セッション数と経過秒数を端末へ定期出力する（AC 5.1）
  - タイムアウトは設定値を参照する（無限待機に対応）
  - _Requirements: 2.1, 2.3, 2.4, 4.2, 4.3, 4.4, 5.1_

- [ ] 3.2 StopUseCase のユニットテストを更新・追加する
  - 既存テストの `execute()` 呼び出しを `execute(false)` に更新する
  - force 停止で SIGTERM をスキップして SIGKILL が呼ばれることをテストする
  - 無限待機設定時のタイムアウトループ動作を確認する
  - _Requirements: 2.1, 4.3, 5.1_

- [ ] 4. Application 層: PollingUseCase の graceful shutdown 改修
- [ ] 4.1 PollingUseCase のシグナル受信ロジックと ShutdownMode 遷移を改修する
  - コンストラクタで shutdown タイムアウト設定を受け取れるようにする
  - SIGINT 複数回受信に対応する（1 回目: graceful 遷移、2 回目: force 遷移）
  - SIGTERM 受信で graceful モードへ遷移し、二重受信は無視してログを出力する
  - _Requirements: 1.1, 1.2, 2.2, 5.4_

- [ ] 4.2 graceful_shutdown() を ShutdownMode に対応させる
  - graceful モードの場合: 全セッション完了を待ち、タイムアウト経過時に強制 kill する
  - force モードの場合: 即座に全セッションを kill して最大 5 秒回収を待つ
  - 5 秒ごとに残セッション数と経過秒数をログ出力する（AC 1.5）
  - セッション状態ファイルへアクティブセッション数を定期書き込みする（stop CLI からポーリングされる）
  - 全セッション完了時・タイムアウト強制終了時にそれぞれログを記録する
  - _Requirements: 1.3, 1.4, 1.5, 2.3, 5.2, 5.3_

- [ ] 4.3 PollingUseCase の統合テストを追加する
  - SIGTERM 受信後に新規セッション起動が停止することを確認する
  - graceful_shutdown でタイムアウト後に全セッション強制終了が呼ばれることを確認する
  - 2 回目 SIGINT 受信後に即座に全セッション強制終了が呼ばれることを確認する
  - _Requirements: 1.1, 1.2, 1.3, 2.2, 2.3_

- [ ] 5. Adapter 層: CLI `--force` フラグ追加
- [ ] 5.1 stop サブコマンドに `--force` フラグを追加する
  - stop コマンドに force オプションを追加し、ユーザーが graceful/force を選択できるようにする
  - bootstrap の stop ハンドラで force 値を StopUseCase へ渡す
  - bootstrap で StopUseCase 生成時に設定値の shutdown タイムアウトを渡す
  - _Requirements: 4.1, 4.2, 4.3, 2.1_

- [ ] 6. Application/Outbound 層: PidFilePort のセッション状態拡張
- [ ] 6.1 PidFilePort にセッション状態ファイルの読み書き機能を追加する
  - アクティブセッション数の書き込みと読み取りを行えるようにする
  - デーモン終了時にセッション状態ファイルを削除する
  - _Requirements: 2.4, 5.1_

- [ ] 7. Bootstrap 層: PollingUseCase への shutdown_timeout 注入
- [ ] 7.1 PollingUseCase 生成時に設定値を渡す
  - フォアグラウンド・デーモン両モードで shutdown タイムアウト設定を PollingUseCase へ注入する
  - StopUseCase 生成時のハードコードタイムアウトを設定値へ置き換える
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_
