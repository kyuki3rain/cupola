# 実装計画

- [ ] 1. ドメイン型とエンティティの拡張
- [ ] 1.1 (P) PR レベルレビューを表す型を追加する
  - PR レベルレビュー 1 件を表す型（投稿日時・本文・レビュー状態・author 情報を保持）を追加する
  - レビュー状態（COMMENTED / CHANGES_REQUESTED）を表す enum を追加する
  - 既存のコード行コメント型と同一ファイル・同一セクションに定義し、スタイルを揃える
  - ユニットテスト: 型の構築と各フィールドアクセス
  - _Requirements: 1.1, 1.2_

- [ ] 1.2 (P) PR 観測結果に PR レベルレビューのリストを追加する
  - PR 観測結果の構造体に「PR レベルレビューのリスト」フィールドを追加する
  - PR 観測インターフェースのシグネチャは変更せず、戻り値の内部構造のみ拡張する
  - フィールド追加によるコンパイルエラーをすべて修正する
  - _Requirements: 1.1_

- [ ] 1.3 (P) Issue エンティティに「最終 PR レビュータイムスタンプ」フィールドを追加する
  - Issue エンティティに `last_pr_review_submitted_at: Option<DateTime<Utc>>` フィールドを追加する
  - デフォルト値は `None`（既存レコードとの後方互換性を保つ）
  - メタデータ更新用の構造体にも同フィールドを追加する
  - フィールド追加によるコンパイルエラー・clippy 警告をすべて修正する
  - _Requirements: 2.1, 2.3_

- [ ] 2. DBスキーマ拡張とリポジトリ実装
- [ ] 2.1 issues テーブルに `last_pr_review_submitted_at` カラムを追加する
  - 既存マイグレーションパターンに合わせ、`run_add_column_migration(&conn, "ALTER TABLE issues ADD COLUMN last_pr_review_submitted_at TEXT")` を追加する
  - 生の `ALTER TABLE ... ADD COLUMN ...` を `init_schema()` に直接書かず、`duplicate column name` のみを無視する既存実装に従う
  - 統合テスト: 既存スキーマに対して再度初期化してもエラーにならず、マイグレーション後は `last_pr_review_submitted_at` が NULL で初期化されることを確認する（インメモリ DB）
  - _Requirements: 2.5_

- [ ] 2.2 リポジトリの読み書きロジックを拡張する
  - DB レコードから Issue エンティティへの変換で `last_pr_review_submitted_at` を読み取る（TEXT → `Option<DateTime<Utc>>` 変換）
  - メタデータ更新の動的 SQL ビルダーに `last_pr_review_submitted_at` の更新ロジックを追加する（対応フィールドが `Some` の場合のみ）
  - 統合テスト: 書き込み → 読み取りのラウンドトリップ、NULL → Some → NULL の更新
  - _Requirements: 2.1, 2.3_

- [ ] 3. GraphQLクエリの拡張とパースロジック実装
- [ ] 3.1 PR 観測クエリに reviews サブクエリを追加する
  - 既存の PR 観測クエリ文字列に `reviews(first: 100, states: [COMMENTED, CHANGES_REQUESTED]) { nodes { id submittedAt body state author { login } authorAssociation } }` を追加する
  - クエリ文字列の変更のみ（Rust コード変更なし）
  - _Requirements: 1.1_

- [ ] 3.2 PR レベルレビューのパースロジックを実装する
  - GraphQL レスポンスの reviews ノードを走査し、PR レベルレビューのリストを返す関数を実装する
  - `body` が空文字・空白のみのノードをスキップする
  - 投稿日時（`submittedAt`）の ISO8601 パース失敗時はノードをスキップし警告ログを出力する
  - author 関連情報を既存の enum にマップする（不明値は最も制限の強い値にフォールバック）
  - `reviews` フィールド自体が存在しない場合は空リストを返し警告ログを出力する（要件 1.4 対応）
  - ユニットテスト: 正常ケース（COMMENTED / CHANGES_REQUESTED）、除外ケース（body 空、APPROVED、DISMISSED）、パース失敗ケース、フィールド欠損ケース
  - _Requirements: 1.1, 1.2, 1.4_

- [ ] 3.3 PR 観測処理を更新して PR レベルレビューを設定する
  - 既存のコード行コメント取得処理と並行して PR レベルレビューのパースを呼び出す
  - 結果を PR 観測結果の対応フィールドに設定する
  - _Requirements: 1.1_

- [ ] 4. 収集フェーズの拡張
- [ ] 4.1 収集フェーズに PR レベルレビューのフィルタリングロジックを追加する
  - PR 観測結果から、以下の条件で PR レベルレビューをフィルタする：
    - 投稿日時が `issue.last_pr_review_submitted_at.unwrap_or(DateTime::<Utc>::MIN)` より新しい（タイムスタンプ条件）
    - 設定で信頼済みとされている author である（信頼済み author 条件）
  - フィルタ後の新規レビューが存在する場合、`has_review_comments = true` に統合する（既存のコード行コメントスレッドチェックと OR 条件）
  - フィルタ後の最新投稿日時を PR スナップショットの対応フィールドに設定する
  - ユニットテスト: タイムスタンプ境界値（以前・以後・NULL・`DateTime::<Utc>::MIN`）、信頼済み author フィルタ、コード行コメントスレッドとの OR 条件
  - _Requirements: 1.3, 2.2, 2.4, 3.1, 3.4, 3.5_

- [ ] 5. 決定フェーズの拡張（MetadataUpdates への設定）
- [ ] 5.1 fixing トリガー時に最終 PR レビュータイムスタンプを更新する
  - `has_review_comments = true` で Fixing に遷移する際、PR スナップショットの最新投稿日時をメタデータ更新構造体の `last_pr_review_submitted_at` フィールドに設定する
  - 最新投稿日時が `None`（PR レベルレビューなし、コード行コメントスレッドのみで fixing トリガー）の場合は設定しない
  - ユニットテスト: PR レベルレビューあり（タイムスタンプ更新）、スレッドのみ（タイムスタンプ不変）、両方あり（最新タイムスタンプが更新）
  - _Requirements: 2.3, 3.2, 3.3_

- [ ] 6. 統合テストと回帰検証
- [ ] 6.1 (P) エンドツーエンドの fixing ループ統合テストを追加する
  - モック GitHub クライアントを使い、PR レベルレビューあり → Fixing 遷移 → タイムスタンプ更新 → 次サイクルで同一レビューをスキップするシナリオを検証する
  - コード行コメントスレッドと PR レベルレビューが同時に存在するシナリオの検証
  - _Requirements: 2.2, 2.3, 3.4_

- [ ] 6.2 (P) 既存テストの回帰確認と修正
  - PR 観測結果・PR スナップショット・Issue 型の変更によるコンパイルエラー・テスト失敗をすべて修正する
  - `cargo clippy -- -D warnings` と `cargo fmt --check` をパスすることを確認する
  - _Requirements: 3.5_
