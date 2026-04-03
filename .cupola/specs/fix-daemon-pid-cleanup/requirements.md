# Requirements Document

## Project Description (Input)
daemon 正常終了時に PID ファイルが削除されない問題の修正

## はじめに

cupola daemon は起動時に PID ファイル（`cupola.pid`）を作成する。現状でも SIGTERM 等のグレースフルシャットダウン経路では `PollingUseCase::graceful_shutdown()` により PID ファイル削除が試行されるが、`run()` が初期化失敗などで早期 return する経路や、ポーリングループ終了後の共通クリーンアップが十分に保証されていない経路では PID ファイルが残存しうる。次回起動時には stale PID として検出・削除されるため動作は継続できるが、不要な処理ステップと紛らわしいログメッセージが発生する。本修正では、既存のグレースフルシャットダウン処理を維持したまま、daemon の終了経路全体で PID ファイル削除をより確実にする。

## 要件

### Requirement 1: daemon 正常終了時の PID ファイルクリーンアップ

**Objective:** cupola 運用者として、daemon が終了した際に、`graceful_shutdown()` を経由する既存経路に加えてその他の終了経路でも PID ファイルが自動的に削除されることを望む。これにより、次回起動時の stale PID 検出ステップが不要となり、ログが清潔に保たれる。

#### Acceptance Criteria

1. daemon child プロセスのポーリングループ（`polling.run().await`）が正常に完了した場合、daemon は PID ファイルを削除する。
2. daemon child プロセスのポーリングループがエラーで終了した場合、daemon は PID ファイルの削除を試みる（削除失敗はエラーとして伝播させない）。
3. `run()` が `graceful_shutdown()` を通らない経路で終了した場合、daemon は PID ファイル削除を試みる。
4. PID ファイルの削除に失敗した場合、daemon はエラーを無視して元の終了結果（`Result`）をそのまま返す。
5. daemon は PID ファイル削除の試行により、終了処理の成功・失敗の判定結果を変えない。

### Requirement 2: 次回起動時の stale PID 検出の回避

**Objective:** cupola 運用者として、daemon を正常停止した後の次回起動時に stale PID 関連のログメッセージが出力されないことを望む。これにより、ログの可読性が向上する。

#### Acceptance Criteria

1. daemon が正常終了した後に `cupola start` コマンドを実行した場合、stale PID クリーンアップに関するログメッセージは出力されない。
2. daemon が PID ファイルを正常に削除している場合、次回起動時の PID ファイル読み込み結果は `Ok(None)` となる（PID ファイルが存在しない状態）。
