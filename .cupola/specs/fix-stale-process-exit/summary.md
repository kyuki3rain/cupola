# fix-stale-process-exit サマリー

## Feature
`step3_process_exit_check` で既に遷移済みの Issue に対して stale な `ProcessFailed` / `RetryExhausted` イベントが発行されるバグを修正。

## 要件サマリ
1. セッション終了処理前に Issue 最新状態を取得し `needs_process()` ガード適用。false なら INFO ログ (`"ignoring stale process exit"`) を出力しスキップ。
2. stale exit では `retry_count` をインクリメントしない。`ProcessFailed` / `RetryExhausted` は `needs_process()` 状態でのみ発行。
3. PR マージ・Issue クローズ・stall kill の競合シナリオで stale を個別評価。

## アーキテクチャ決定
- ガードは `step3_process_exit_check` ループ内、`handle_successful_exit` / `handle_failed_exit` 呼び出し直前の 1 箇所に置き、成功/失敗両パスを一括カバー。
- `needs_process()` 単独ガードでは `DesignRunning → ImplementationRunning` のような遷移（両方 true）を検出できないため、`SessionManager` の `registered_state`（登録時状態）と現在の DB 状態を比較。判定式: `session.registered_state != issue.state || !issue.state.needs_process()`。
- ガード位置は `record_start` 呼び出し後のため stale セッションでも start ログが残る（軽微、別 Issue の fix-execution-log-started-at で解決）。
- `needs_process()` は既存の純粋ドメイン関数を再利用（`DesignRunning|DesignFixing|ImplementationRunning|ImplementationFixing` で true）。

## コンポーネント
- `application/polling_use_case.rs` `step3_process_exit_check`
- `domain/state.rs` `State::needs_process()`（再利用）
- `application/session_manager.rs` `registered_state`（利用）

## 主要インターフェース
- `State::needs_process() -> bool`（既存）
- `ExitedSession.registered_state: State`

## 学び/トレードオフ
- `handle_successful_exit` には既に内部 `needs_process()` チェックがあったが `handle_failed_exit` に無く非対称だった。外側で一括ガードすることで将来のハンドラ追加時の漏れも防止。
- 根本対処（SessionManager で kill 済みセッションをキャンセルマーク付け）は変更範囲が大きく別 Issue に。
- stale ログは INFO レベル（WARN ではない）として正常系扱い。
