# Implementation Plan

- [x] 1. step3 に stale exit ガード条件を追加する

- [x] 1.1 プロセス終了後、issue 状態が処理不要な場合にスキップするガードを実装する
  - `step3_process_exit_check` のループ内で、issue を取得した後かつハンドラ呼び出しより前に `needs_process()` チェックを追加する
  - `needs_process()` が false の場合、`issue_id` と `state` を含む INFO ログ（`"ignoring stale process exit"`）を出力して `continue` で次セッションへスキップする
  - ガードは成功終了（`handle_successful_exit`）と失敗終了（`handle_failed_exit`）の両方の呼び出し前に一括適用する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3_

- [x] 2. stale exit スキップのユニットテストを追加する

- [x] 2.1 needs_process が false の issue に対して失敗終了がイベントを発行しないことを検証する
  - `DesignReviewWaiting`・`Completed`・`Cancelled` など `needs_process()` が false の各状態を持つ issue でモックを構築する
  - 失敗終了セッションを渡したとき、`ProcessFailed` も `RetryExhausted` も events ベクタに追加されないことをアサートする
  - INFO ログが出力されることを確認する（ログキャプチャ可能な場合）
  - _Requirements: 1.2, 1.3, 2.1, 2.2, 2.3_

- [x] 2.2 PR マージ・Issue クローズ後の競合シナリオでスキップが正常動作することを検証する
  - PR マージにより issue が `ImplementationRunning` から遷移済みの状態でモックを構築する
  - Issue クローズにより issue が `Cancelled` または `Completed` になった場合のモックを構築する
  - 複数セッションが同時に stale 状態で終了しても、各セッションが個別に評価されてすべてスキップされることを確認する
  - 同一サイクルで `needs_process()` が true の issue は従来通りイベントが発行されることも併せて確認する
  - _Requirements: 3.1, 3.2, 3.3_
