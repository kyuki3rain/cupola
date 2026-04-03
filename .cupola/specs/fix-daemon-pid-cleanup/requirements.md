# Requirements Document

## Project Description (Input)
daemon 正常終了時に PID ファイルが削除されない問題の修正

## はじめに

cupola daemon は起動時に PID ファイル（`cupola.pid`）を作成するが、SIGTERM 等によるグレースフルシャットダウン後も PID ファイルが残存する。次回起動時に stale PID として検出・削除されるため動作は継続できるが、不要な処理ステップと紛らわしいログメッセージが発生する。本修正では、daemon の正常終了時に PID ファイルを確実に削除する。

## 要件

### Requirement 1: daemon 正常終了時の PID ファイルクリーンアップ

**Objective:** cupola 運用者として、daemon が正常終了した際に PID ファイルが自動的に削除されることを望む。これにより、次回起動時の stale PID 検出ステップが不要となり、ログが清潔に保たれる。

#### Acceptance Criteria

1. When daemon child プロセスのポーリングループ（`polling.run().await`）が正常に完了した, the daemon shall PID ファイルを削除する。
2. When daemon child プロセスのポーリングループがエラーで終了した, the daemon shall PID ファイルを削除しようとする（削除失敗はエラーとして伝播させない）。
3. If PID ファイルの削除に失敗した, the daemon shall エラーを無視してポーリングループの終了結果（`Result`）をそのまま返す。
4. The daemon shall PID ファイル削除の試行により、ポーリングループの成功・失敗の判定結果を変えない。

### Requirement 2: 次回起動時の stale PID 検出の回避

**Objective:** cupola 運用者として、daemon を正常停止した後の次回起動時に stale PID 関連のログメッセージが出力されないことを望む。これにより、ログの可読性が向上する。

#### Acceptance Criteria

1. When daemon が正常終了した後に cupola start コマンドを実行した, the daemon shall stale PID クリーンアップのログメッセージを出力しない。
2. While daemon が PID ファイルを正常に削除した状態, the daemon shall 次回起動時の PID ファイル読み込みで `Ok(None)` を返す（PID ファイルが存在しない状態）。
