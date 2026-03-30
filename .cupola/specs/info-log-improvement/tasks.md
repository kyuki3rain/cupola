# Implementation Plan

- [x] 1. 状態遷移の INFO ログを追加する
  - `TransitionUseCase::apply()` 内で、DB 更新（`update_state`）が成功し状態遷移が永続化された直後に、遷移元状態・遷移先状態・issue_number を構造化フィールドとして含む INFO ログを出力する
  - `State` enum が `Display` trait を実装していない場合は `Debug` フォーマットを使用する
  - IssueClosed → Cancelled を含む全ての「update_state 成功パス」が同一のログ出力を通ることを確認する（DB 更新に失敗したパスではログを出力しない）
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 2. PR 作成の INFO ログを追加する
- [x] 2.1 (P) 新規 PR 作成成功時のログを追加する
  - `create_pr_from_output()` 内で GitHub API による PR 作成が成功した直後に、pr_number・head ブランチ・base ブランチを含む INFO ログを出力する
  - _Requirements: 2.1_

- [x] 2.2 (P) 既存 PR 検出によるスキップ時のログを追加する
  - DB に既存 PR 番号がある場合と、GitHub 上に既存 PR が見つかった場合の 2 パスで、スキップした旨と pr_number を含む INFO ログを出力する
  - _Requirements: 2.2_

- [x] 3. fixing 後処理の INFO ログを追加する
- [x] 3.1 (P) スレッド返信・resolve 成功時の個別ログを追加する
  - `process_fixing_output()` 内のループで、スレッド返信が成功した直後に thread_id を含む INFO ログを出力する
  - スレッド resolve が成功した直後にも同様に thread_id を含む INFO ログを出力する
  - _Requirements: 3.2, 3.3_

- [x] 3.2 (P) fixing 後処理完了のサマリーログを追加する
  - `process_fixing_output()` のループ完了後に、処理したスレッド数と issue_number を含む INFO ログを出力する
  - _Requirements: 3.1_

- [x] 4. Claude Code プロセス終了の INFO ログを追加する
  - `step3_process_exit_check()` 内の成功パスで、exit_code と issue_number を含む INFO ログを出力する
  - 失敗パスでも同様に、exit_code・issue_number・失敗の旨を含む INFO ログを出力する
  - _Requirements: 4.1, 4.2_

- [x] 5. ログ出力の検証テストを追加する
  - 状態遷移ログのユニットテスト: `TransitionUseCase::apply()` 呼び出し後に期待するフィールド（from, to, issue_number）が INFO ログに含まれることを検証する
  - PR 作成・fixing 完了・プロセス終了ログの統合テスト: 各フローを実行し、期待するログメッセージとフィールドが出力されることを検証する
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 3.1, 3.2, 3.3, 4.1, 4.2_
