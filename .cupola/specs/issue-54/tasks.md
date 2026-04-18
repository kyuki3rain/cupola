# 実装計画

- [ ] 1. ドメイン型とエンティティの拡張
- [ ] 1.1 (P) `PrLevelReview` 型と `PrReviewState` enum を追加する
  - `src/application/port/github_client.rs` に `PrLevelReview` 構造体と `PrReviewState` enum を定義する
  - フィールド: `id: String`, `submitted_at: DateTime<Utc>`, `body: String`, `state: PrReviewState`, `author: String`, `author_association: AuthorAssociation`
  - `ReviewThread` と同一ファイル・同一セクションに定義し、命名・スタイルを揃える
  - ユニットテスト: 構造体の構築と各フィールドアクセス
  - _Requirements: 1.1, 1.2_

- [ ] 1.2 (P) `PrObservation` に `pr_level_reviews` フィールドを追加する
  - 既存の `PrObservation` 構造体に `pr_level_reviews: Vec<PrLevelReview>` を追加する
  - `GitHubClient` trait の `observe_pr()` シグネチャは変更不要（戻り値型の内部変更のみ）
  - 既存コードのコンパイルエラーを修正（フィールド追加によるパターンマッチ等）
  - _Requirements: 1.1_

- [ ] 1.3 (P) `Issue` エンティティに `last_pr_review_submitted_at` フィールドを追加する
  - `src/domain/` 内の `Issue` 構造体に `last_pr_review_submitted_at: Option<DateTime<Utc>>` を追加する
  - デフォルト値は `None`
  - `MetadataUpdates` 構造体にも同フィールドを追加する
  - 既存コードのコンパイルエラー・clippy 警告をすべて修正する
  - _Requirements: 2.1, 2.3_

- [ ] 2. DBスキーマ拡張とリポジトリ実装
- [ ] 2.1 `issues` テーブルに `last_pr_review_submitted_at` カラムを追加する
  - `src/adapter/outbound/sqlite_connection.rs` の `init_schema()` に `ALTER TABLE issues ADD COLUMN last_pr_review_submitted_at TEXT` を追加する
  - 既存の `ALTER TABLE` パターン（カラム存在時のエラー無視）に従う
  - 統合テスト: マイグレーション後に NULL で初期化されることを確認（インメモリ DB）
  - _Requirements: 2.5_

- [ ] 2.2 `SqliteIssueRepository` の読み書きロジックを拡張する
  - `row_to_issue()` に `last_pr_review_submitted_at` の読み取り（TEXT → `Option<DateTime<Utc>>` 変換）を追加する
  - `update_state_and_metadata()` の動的 SQL ビルダーに `last_pr_review_submitted_at` の更新ロジックを追加する（`MetadataUpdates` フィールドが `Some` の場合のみ）
  - 統合テスト: 書き込み → 読み取りのラウンドトリップ、NULL → Some → NULL の更新
  - _Requirements: 2.1, 2.3_

- [ ] 3. GraphQLクエリの拡張とパースロジック実装
- [ ] 3.1 `OBSERVE_PR_QUERY` に `reviews` サブクエリを追加する
  - `src/adapter/outbound/github_graphql_client.rs` の `OBSERVE_PR_QUERY` に `reviews(first: 100, states: [COMMENTED, CHANGES_REQUESTED]) { nodes { id submittedAt body state author { login } authorAssociation } }` を追加する
  - クエリ文字列の変更のみ（Rust コード変更なし）
  - _Requirements: 1.1_

- [ ] 3.2 `parse_pr_level_reviews()` 関数を実装する
  - GraphQL レスポンスの `reviews.nodes` を走査し `Vec<PrLevelReview>` を返す関数を実装する
  - `body` が空文字・空白のみのノードをスキップする
  - `submittedAt` の ISO8601 パース失敗時はノードをスキップし警告ログを出力する
  - `authorAssociation` を既存 `AuthorAssociation` enum にマップする（不明値は最も制限の強い値にフォールバック）
  - `reviews` フィールド自体が存在しない場合は空 Vec を返し警告ログを出力する（要件 1.4 対応）
  - ユニットテスト: 正常ケース（COMMENTED / CHANGES_REQUESTED）、除外ケース（body 空、APPROVED、DISMISSED）、パース失敗ケース、フィールド欠損ケース
  - _Requirements: 1.1, 1.2, 1.4_

- [ ] 3.3 `observe_pr()` の実装を更新して `pr_level_reviews` を設定する
  - 既存の `parse_review_thread()` 呼び出しと並行して `parse_pr_level_reviews()` を呼び出す
  - 結果を `PrObservation.pr_level_reviews` に設定する
  - _Requirements: 1.1_

- [ ] 4. 収集フェーズの拡張
- [ ] 4.1 `collect.rs` に PR レベルレビューのフィルタリングロジックを追加する
  - `observe_pr_for_type()` 内で `observation.pr_level_reviews` を以下の条件でフィルタする：
    - `submitted_at > issue.last_pr_review_submitted_at.unwrap_or(DateTime::<Utc>::MIN)` （タイムスタンプ条件）
    - `config.is_comment_trusted(&r.author_association, &r.author)` （信頼済み author 条件）
  - フィルタ後の新規レビューが存在する場合、`has_review_comments = true` に統合する（既存の `unresolved_threads` チェックと OR 条件）
  - フィルタ後の最新 `submitted_at` を `PrSnapshot.newest_pr_review_submitted_at` に設定する
  - ユニットテスト: タイムスタンプ境界値（以前・以後・NULL・UNIX_EPOCH）、TrustedAssociations フィルタ、unresolved_threads との OR 条件
  - _Requirements: 1.3, 2.2, 2.4, 3.1, 3.4, 3.5_

- [ ] 5. 決定フェーズの拡張（MetadataUpdates への設定）
- [ ] 5.1 fixing トリガー時に `last_pr_review_submitted_at` を MetadataUpdates に設定する
  - `src/domain/decide.rs` の `decide_design_review_waiting()` と `decide_implementation_review_waiting()` において、`has_review_comments = true` で Fixing に遷移する際、`MetadataUpdates.last_pr_review_submitted_at = snap.[design|impl]_pr.newest_pr_review_submitted_at` を設定する
  - `newest_pr_review_submitted_at` が `None`（PR レベルレビューなし、thread のみで fixing トリガー）の場合は `MetadataUpdates.last_pr_review_submitted_at` を設定しない
  - ユニットテスト: PR レベルレビューあり（タイムスタンプ更新）、thread のみ（タイムスタンプ不変）、両方あり（最新タイムスタンプが更新）
  - _Requirements: 2.3, 3.2, 3.3_

- [ ] 6. 統合テストと回帰検証
- [ ] 6.1 (P) エンドツーエンドの fixing ループ統合テストを追加する
  - モック `GitHubClient` を使い、PR レベルレビューあり → Fixing 遷移 → `last_pr_review_submitted_at` 更新 → 次サイクルで同一レビューをスキップするシナリオを検証する
  - `reviewThreads` と PR レベルレビューが同時に存在するシナリオの検証
  - _Requirements: 2.2, 2.3, 3.4_

- [ ] 6.2 (P) 既存テストの回帰確認と修正
  - `PrObservation`・`PrSnapshot`・`Issue` の型変更によるコンパイルエラー・テスト失敗をすべて修正する
  - `cargo clippy -- -D warnings` と `cargo fmt --check` をパスすることを確認する
  - _Requirements: 3.5_
