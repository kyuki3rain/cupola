# ADR-001: step5_stall_detection でイベントを即時生成しない理由

## ステータス

Accepted

## コンテキスト

`step5_stall_detection` では、stall kill（停止プロセスの強制終了）が発生した後に、後続のイベントをどのタイミングで処理するかという設計上の選択があった。

**方式A（即時 push）**: stall kill 後、`step5_stall_detection` 内で直接 `ProcessFailed` イベントを即時 push する。  
**方式B（次サイクル委譲）**: stall kill 後はイベントを生成せず、次の実行サイクルの `step3` でイベントを一元処理する。

## 決定

stall kill 後のイベントは次サイクルの `step3` で一元処理する（方式B を採用）。

## 理由

方式A（即時 push）を採用した場合、以下のバグが発生することが確認された：

1. `step5_stall_detection` が stall kill 後に `ProcessFailed` を即時 push すると、ステート遷移が `ProcessFailed → DesignRunning → DesignRunning` という自己遷移を引き起こす。
2. この自己遷移により、stale exit guard が無効化される。
3. stale exit guard が無効化された状態で次サイクルに入ると、`step3` で `ProcessFailed` イベントが再発火する。
4. 結果として `retry_count` が二重インクリメントされ、リトライ回数が意図より早く上限に達するバグが生じる。

方式B では、stall kill 後のイベント処理を `step3` に委譲することで、イベント処理の一元化と stale exit guard の正常動作を両立できる。

## 却下した代替案

**方式A: `step5_stall_detection` で `ProcessFailed` を即時 push する方式**  
却下理由: 上記の通り、`ProcessFailed → DesignRunning → DesignRunning` の自己遷移が発生し、stale exit guard が無効化されて `retry_count` が二重インクリメントされるバグが再現したため。
