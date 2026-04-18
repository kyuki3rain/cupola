# リサーチ・設計決定記録

---
**Feature**: issue-54 — PRレベルのレビューコメントを fixing トリガーとして拾う
**Discovery Scope**: Extension（既存システムへの機能追加）
**Key Findings**:
- GitHub GraphQL の `reviews` フィールドで PR レベルレビューを取得できる
- `reviewThreads` と違い resolve の概念がなく、状態管理が必要
- 既存の `PrObservation`・`PrSnapshot`・`Issue` エンティティを最小限の変更で拡張できる
- DB に `last_pr_review_submitted_at` カラムを追加し、既存の `ALTER TABLE IF NOT EXISTS` パターンでマイグレーションできる

---

## リサーチログ

### GitHub GraphQL: PR レベルレビューの取得方法

- **Context**: `reviewThreads` クエリは code-line コメントのみ。PR body コメントは別フィールドが必要。
- **Sources Consulted**: 既存の `OBSERVE_PR_QUERY`（`github_graphql_client.rs` 33-69行）、GitHub GraphQL スキーマ
- **Findings**:
  - `pullRequest.reviews(first: N, states: [COMMENTED, CHANGES_REQUESTED])` で取得可能
  - フィールド: `id`, `databaseId`, `submittedAt`, `body`, `state`, `author { login }`, `authorAssociation`
  - `state` は `PENDING` / `COMMENTED` / `APPROVED` / `CHANGES_REQUESTED` / `DISMISSED`
  - 対象は `COMMENTED` と `CHANGES_REQUESTED`（body が空でないもの）
  - ページネーションは最大 100 件/リクエスト（PRの性質上超えることは稀）
- **Implications**: 既存の `OBSERVE_PR_QUERY` を拡張し、`reviews` サブクエリを追加する。または専用のクエリを追加する。

### 既存コードの統合ポイント

- **Context**: どのファイルをどの程度変更するか把握するため
- **Findings**:
  - `PrObservation`（`src/application/port/github_client.rs` 77-82行）: 拡張ポイント
  - `PrSnapshot`（`src/application/polling/collect.rs`）: `has_review_comments` の計算ロジック
  - `Issue` エンティティ（`src/domain/`）: `last_pr_review_submitted_at` フィールド追加
  - `MetadataUpdates`（`src/application/`）: タイムスタンプ更新用フィールド追加
  - DB スキーマ（`src/adapter/outbound/sqlite_connection.rs` 57-127行）: `ALTER TABLE` 追加
  - `decide.rs`（`src/domain/decide.rs`）: `has_review_comments` は既にカバー済み（変更不要の可能性）
- **Implications**: 変更範囲は adapter と domain/application 境界に集中し、decide.rs の変更は最小限にできる

### 処理済みレビューのトラッキング戦略

- **Context**: resolve がないため「既に対応済み」の判定基準が必要
- **Alternatives Considered**:
  1. DB タイムスタンプ記録（fixing トリガー時に更新）
  2. DB タイムスタンプ記録（fixing 成功時に更新）
  3. fixing 後に Cupola がリプライを投稿し、次回はそのタイムスタンプ以降のみを対象とする
- **Findings**:
  - 案1: シンプル。fixing 失敗時も同一レビューで再トリガーしない。既存 ci_fix_count パターンと同様の「楽観的」アプローチ。
  - 案2: fixing 成功時まで再トリガー可能。ただし、タイムスタンプを SpawnProcess エフェクトに載せて resolve 層まで伝搬させる必要があり、実装が複雑。
  - 案3: リプライ投稿後のタイムスタンプ管理が必要で、案1と同等の DB 記録が結局必要。
- **Selected**: 案1（fixing トリガー時に更新）。理由は後述。

---

## アーキテクチャパターン評価

| 案 | 説明 | 利点 | リスク・制限 |
|----|------|------|-------------|
| OBSERVE_PR_QUERY 拡張 | 既存クエリに `reviews` サブクエリを追加 | 1往復で完結、既存パターン踏襲 | クエリが長くなる |
| 専用クエリ追加 | `list_pr_level_reviews()` を GitHubClient trait に追加 | 責務が明確 | API 呼び出し回数が増える |
| タイムスタンプ: trigger 時更新 | fixing 採用時に DB 更新 | シンプル、ci_fix_count と対称 | 同一レビューの再トリガー不可 |
| タイムスタンプ: success 時更新 | SpawnProcess にタイムスタンプを載せ resolve 層で更新 | 正確な成功判定 | エフェクト設計の複雑化 |

---

## 設計決定

### 決定: OBSERVE_PR_QUERY に reviews サブクエリを追加する

- **Context**: PR ごとに review threads + reviews の両方を取得する必要がある
- **Alternatives Considered**:
  1. 既存 `OBSERVE_PR_QUERY` を拡張
  2. 専用クエリを追加して `GitHubClient` trait に新メソッド
- **Selected Approach**: 既存 `OBSERVE_PR_QUERY` を拡張し、`reviews` サブクエリを追加する
- **Rationale**: 追加の API 呼び出しなしで完結する。PR あたりのレビュー数は 100 件以内に収まる前提で first: 100 で十分。
- **Trade-offs**: クエリが若干長くなるが、既存のパースロジックに素直に追加できる
- **Follow-up**: 実装時に 100 件超のページネーション要否を再確認

### 決定: タイムスタンプを fixing トリガー時に更新する

- **Context**: PR レベルレビューには resolve がなく、処理済み判定の基準が必要
- **Alternatives Considered**:
  1. トリガー時更新（楽観的）
  2. 成功時更新（正確だが複雑）
- **Selected Approach**: `MetadataUpdates` に `last_pr_review_submitted_at` を追加し、decide.rs が fixing をトリガーする際に更新する
- **Rationale**: 既存の ci_fix_count パターンと対称的でシンプル。fixing 失敗時に同一レビューで再ループしないことは許容される（ユーザーが新しいコメントを投稿すれば再トリガーされる）
- **Trade-offs**: fixing 失敗時に自動リトライはされないが、無限ループを防ぐ安全側の設計
- **Follow-up**: 将来的に「失敗後に再トリガーするオプション」を設けることも検討可能

### 決定: PrLevelReview を port 層の型として定義する

- **Context**: 新しい型をどのレイヤーに置くか
- **Alternatives Considered**:
  1. domain 層に配置
  2. application/port 層に配置（ReviewThread と同じ場所）
- **Selected Approach**: `ReviewThread` と同じく `src/application/port/github_client.rs` 内に定義する
- **Rationale**: GitHub API 固有の概念であり、ドメインロジックには含まれない。ReviewThread と同じ慣習に従う

---

## リスクと緩和策

- `reviews` フィールドは `reviewThreads` と独立して動作するため、両者の整合性は GitHub 側で保証される
- `last_pr_review_submitted_at` の型を `Option<DateTime<Utc>>` とし NULL 許容にすることで、既存レコードの互換性を保つ
- `OBSERVE_PR_QUERY` のクエリ拡張がパースロジックに影響しないよう、新フィールドのパースは独立した関数に切り出す

---

## 参照

- 既存実装: `src/adapter/outbound/github_graphql_client.rs`（OBSERVE_PR_QUERY, parse_review_thread）
- 既存実装: `src/application/polling/collect.rs`（has_review_comments の計算ロジック）
- 既存実装: `src/domain/decide.rs`（decide_design_review_waiting, decide_implementation_review_waiting）
- 既存実装: `src/adapter/outbound/sqlite_connection.rs`（DB スキーマ、マイグレーション）
- GitHub GraphQL API: `pullRequest.reviews` フィールド（states, submittedAt, body, authorAssociation）
