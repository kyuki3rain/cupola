# 要件定義書

## はじめに

本要件は Issue #54「PRレベルのレビューコメントをfixingトリガーとして拾う」に対応する。

現在 cupola は `reviewThreads`（コード行に紐づくコメント）のみを fixing トリガーとして検知している。
本機能では、PR レベルのレビューコメント（`reviews` フィールド経由で取得できる body 付きレビュー）も検知対象とし、
fixing ループに組み込む。

resolve の概念がない PR レベルレビューについては、DB に「最後に確認したレビューのタイムスタンプ」を記録し、
新しいレビューのみを fixing トリガーとして扱う。

---

## 要件

### 要件 1: PRレベルレビューの取得

**目的:** 自動化エージェントとして、コード行に紐づかない PR レベルのレビューコメントを取得できるようにしたい。
これにより、レビュアーが body 付きレビューで指摘した問題も自動修正の対象となる。

#### 受け入れ基準

1. When PRレベルのレビュー取得が要求されたとき、the cupola shall GraphQL `reviews` フィールドを使用して当該PRのレビュー一覧（`id`、`submittedAt`、`body`、`state`、`author.login`、`authorAssociation` を含む）を取得する
2. The cupola shall `state` が `COMMENTED` または `CHANGES_REQUESTED` であり、かつ `body` が空でないレビューのみを処理対象とする
3. The cupola shall 設定済みの `TrustedAssociations` に基づき、信頼されていない author によるレビューを除外する
4. If GraphQL クエリが失敗したとき、the cupola shall エラーをログに記録し、当該サイクルの PR レベルレビュー検知をスキップする（既存の `reviewThreads` 検知には影響を与えない）

---

### 要件 2: 処理済みレビューの管理

**目的:** 自動化エージェントとして、既に処理したレビューを再度 fixing トリガーとして拾わないようにしたい。
これにより、同一レビューによる無限ループを防ぐ。

#### 受け入れ基準

1. The cupola shall Issue ごとに「最後に fixing トリガーとして採用した PR レベルレビューの `submittedAt` タイムスタンプ」を DB に保持する
2. When PRレベルレビューを fixing トリガーとして評価するとき、the cupola shall `submittedAt` が保存済みタイムスタンプ以前のレビューを除外し、それより新しいレビューのみを対象とする
3. When fixing トリガーとして新しい PR レベルレビューを採用したとき、the cupola shall 当該レビュー群の最新 `submittedAt` タイムスタンプを DB に記録する
4. While 保存済みタイムスタンプが存在しないとき、the cupola shall すべての PR レベルレビューを新規として扱う
5. The cupola shall DB カラム `last_pr_review_submitted_at`（TEXT 型、NULL 許容）を `issues` テーブルに追加し、既存レコードは NULL で初期化する

---

### 要件 3: 既存 fixing フローへの統合

**目的:** 自動化エージェントとして、PR レベルレビューを既存の fixing ワークフロー（`reviewThreads` による検知と同等）に統合したい。
これにより、コメントの種類に依らず一貫した自動修正が行われる。

#### 受け入れ基準

1. When 新しい PR レベルレビューが存在するとき、the cupola shall `reviewThreads` による `has_review_comments` と同様に fixing をトリガーし、`FixingProblemKind::ReviewComments` を原因として付与する
2. When 設計 PR に新しい PR レベルレビューが存在するとき、the cupola shall `DesignReviewWaiting` から `DesignFixing` への遷移をトリガーする
3. When 実装 PR に新しい PR レベルレビューが存在するとき、the cupola shall `ImplementationReviewWaiting` から `ImplementationFixing` への遷移をトリガーする
4. When `reviewThreads` による未解決コメントと PR レベルレビューが同時に存在するとき、the cupola shall 双方を合わせて単一の fixing セッションとして扱う
5. If 新しい PR レベルレビューが存在しないとき、the cupola shall 既存の `reviewThreads` 検知ロジックに影響を与えない
