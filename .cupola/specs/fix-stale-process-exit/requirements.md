# Requirements Document

## Project Description (Input)
step3_process_exit_check で stale process exit が既に遷移済みの issue に誤ってイベントを発行するバグの修正。`needs_process()` チェックを追加して stale な exit を無視する。

## はじめに

Cupola のポーリングループは 10 ステップで構成されており、step3 ではプロセスの終了を検知してイベントを生成する。しかし現状、step3 はプロセス終了を検知した時点での issue の状態を確認せず `ProcessFailed` / `RetryExhausted` イベントを無条件に発行する。このため、PR マージや Issue クローズなどの別イベントにより issue が既に別状態へ遷移済みである場合でも stale な exit が誤ったイベントを発行し、`retry_count` の不正インクリメントやノイズログを引き起こす。

本修正は `step3_process_exit_check` に `needs_process()` ガードを追加し、stale な exit を安全に無視することで上記の問題を解消する。

## Requirements

### Requirement 1: stale process exit の無視

**Objective:** Cupola オペレーターとして、既に遷移済みの issue に対して stale な process exit がイベントを発行しないようにしたい。そうすることで、`retry_count` が不正にインクリメントされず、ノイズログが発生しないようにする。

#### Acceptance Criteria

1. When `step3_process_exit_check` が終了したセッションを処理する際、the PollingUseCase shall 対象 issue の最新状態を取得してから `handle_successful_exit` / `handle_failed_exit` を呼び出す
2. If issue の状態が `needs_process()` を満たさない場合（`DesignRunning`・`DesignFixing`・`ImplementationRunning`・`ImplementationFixing` のいずれでもない場合）, then the PollingUseCase shall そのセッションの処理をスキップし、イベントを発行しない
3. When stale exit がスキップされた場合, the PollingUseCase shall `issue_id` と `state` を含む INFO レベルのログを出力する（例: `"ignoring stale process exit"`）
4. While issue の状態が `needs_process()` を満たす場合, the PollingUseCase shall 従来通り `handle_successful_exit` または `handle_failed_exit` を呼び出してイベントを発行する

### Requirement 2: retry_count の正確性保証

**Objective:** Cupola オペレーターとして、`retry_count` が実際に失敗した試行にのみインクリメントされることを保証したい。そうすることで、リトライポリシーが正確に機能し、誤った `RetryExhausted` 遷移が発生しないようにする。

#### Acceptance Criteria

1. If stale な process exit が検知された場合, then the PollingUseCase shall `retry_count` をインクリメントしない
2. When `ProcessFailed` イベントが発行される場合, the PollingUseCase shall issue が `needs_process()` 状態にある場合に限定する
3. The PollingUseCase shall `RetryExhausted` イベントを、issue が `needs_process()` 状態にない場合には発行しない

### Requirement 3: 競合シナリオへの対応

**Objective:** Cupola オペレーターとして、PR マージ・Issue クローズ・stall kill が同一ポーリングサイクル内で競合する場合でも正確なイベント処理が行われることを保証したい。

#### Acceptance Criteria

1. When PR マージにより issue が `ImplementationRunning` へ遷移した後、前サイクルの stale process exit が step3 で検知される場合, the PollingUseCase shall その exit を無視し `ProcessFailed` を発行しない
2. When Issue クローズにより issue が `Cancelled` または `Completed` へ遷移した後、stale process exit が検知される場合, the PollingUseCase shall その exit をスキップする
3. The PollingUseCase shall 同一ポーリングサイクル内での複数 stale exit を個別に評価し、それぞれ `needs_process()` チェックを適用する
