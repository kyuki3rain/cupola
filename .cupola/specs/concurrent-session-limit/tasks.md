# Implementation Plan

- [ ] 1. Config と設定読み込みに同時実行数上限を追加
- [ ] 1.1 (P) domain 層の Config に同時実行数上限フィールドを追加する
  - Config 値オブジェクトに同時実行セッション数の上限を保持する Optional フィールドを追加する
  - 未設定（None）は制限なしを意味し、0 も制限なしとして扱う
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 1.2 (P) bootstrap 層の設定ローダーで TOML からの読み込みとマッピングを実装する
  - CupolaToml 構造体に同時実行数上限の Optional フィールドを追加し、serde default で後方互換性を維持する
  - into_config() メソッドで CupolaToml から Config への変換にフィールドを追加する
  - 未指定時は None として Config に渡される
  - _Requirements: 1.1, 1.2_

- [ ] 2. SessionManager にセッション数カウント機能を追加
- [ ] 2.1 (P) SessionManager に実行中セッション数を返すメソッドを追加する
  - 内部の HashMap のエントリ数を返す公開メソッドを実装する
  - 戻り値は usize 型で、常に正確な実行中プロセス数を反映する
  - register 後にカウントが増加し、collect_exited 後にカウントが減少することをテストで検証する
  - _Requirements: 4.1, 4.2_

- [ ] 3. PollingUseCase の Step 7 に同時実行数の上限チェックを追加
- [ ] 3.1 step7_spawn_processes のプロセス起動ループ冒頭に上限チェックを追加する
  - ループ内で毎回 SessionManager の実行中セッション数と Config の上限値を比較する
  - 上限以上の場合はログを出力してループを抜ける（残りの Issue は次の polling サイクルで再試行）
  - 上限未設定（None）または 0 の場合はチェックをスキップし、全 Issue に対してプロセスを起動する
  - スキップされた Issue の状態は変更しない（needs_process のまま維持）
  - 上限到達時は info レベルでカウントと上限値をログに記録する
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.2, 3.3_
  - _Contracts: PollingUseCase Service, SessionManager Service_

- [ ] 4. status コマンドに実行中プロセス数と上限の表示を追加
- [ ] 4.1 status コマンドの出力に実行中プロセス数のサマリー行を追加する
  - active issues のうち実行中状態かつプロセス ID を持つものをカウントして表示する
  - 同時実行数上限が設定されている場合は上限値も合わせて表示する（例: "Running: 2/3"）
  - 未設定の場合はカウントのみ表示する（例: "Running: 2"）
  - Config の読み込みは既存の config_loader を再利用する
  - _Requirements: 5.1, 5.2_

- [ ] 5. 統合テストで同時実行数制限の動作を検証
- [ ] 5.1 上限設定時にプロセス起動数が制限されることを検証するテストを作成する
  - 上限を 2 に設定し 3 つの needs_process な Issue がある場合、2 つだけ起動されることを検証する
  - 上限未設定時に全 Issue が起動されることを検証する
  - 上限を 0 に設定した場合に制限なしとして動作することを検証する
  - _Requirements: 2.1, 2.2, 2.3, 3.1_

- [ ]* 5.2 (P) プロセス終了後の再試行動作を検証するテストを作成する
  - 上限到達後にプロセスが終了し空きができた次サイクルで、スキップされていた Issue が起動されることを検証する
  - _Requirements: 3.2_
