# 実装タスク一覧

## Task Format

- `(P)` : 並列実行可能なタスク
- `*` : 省略可能なテストタスク（MVP 後に対応可）

---

- [ ] 1. AuthorAssociation と TrustedAssociations のドメイン型を実装する
  - `AuthorAssociation` enum（Owner / Member / Collaborator / Contributor / FirstTimer / FirstTimeContributor / None）を定義する
  - 文字列からの変換ロジック（大文字小文字不問）と不正値エラーを実装する
  - `TrustedAssociations` enum（`All` / `Specific(Vec<AuthorAssociation>)`）を定義する
  - `TrustedAssociations::is_trusted(&AuthorAssociation) -> bool` メソッドを実装する
  - `TrustedAssociations` のデフォルト値を `Specific([Owner, Member, Collaborator])` とする
  - _Requirements: 1.3, 1.4_

- [ ] 2. trusted_associations 設定フィールドを Config と TOML ローダーに追加する

- [ ] 2.1 (P) Config 構造体と TOML 読み込みロジックを拡張する
  - `Config` 構造体に `trusted_associations: TrustedAssociations` フィールドを追加する
  - `CupolaToml` に `trusted_associations: Option<Vec<String>>` フィールドを追加する
  - `"all"` 文字列を `TrustedAssociations::All` に変換するロジックを実装する
  - 文字列リストを `TrustedAssociations::Specific` に変換するロジックを実装する
  - 省略時は `TrustedAssociations::default()` を使用する
  - タスク 1 の完了が前提（`AuthorAssociation` 型の依存）
  - _Requirements: 1.1, 1.2_

- [ ] 2.2 設定値の検証ロジックを追加する
  - `Config::validate()` に `trusted_associations` の検証を追加する
  - 不正な association 文字列が含まれる場合は `ConfigError` を返す
  - 起動時エラーとしてログ出力し処理を停止させる
  - _Requirements: 1.5_

- [ ] 3. GitHubClient ポートと ReviewComment 構造体を拡張する

- [ ] 3.1 (P) Timeline API・Permission API・ラベル操作メソッドをポートに定義する
  - `fetch_label_actor_login(issue_number, label_name) -> Result<Option<String>>` を `GitHubClient` trait に追加する
  - `fetch_user_permission(username) -> Result<RepositoryPermission>` を `GitHubClient` trait に追加する
  - `RepositoryPermission` enum（Admin / Write / Triage / Read）と `to_author_association()` 変換を定義する
  - `remove_label(issue_number, label_name)` が未実装の場合は追加する
  - `add_issue_comment(issue_number, body)` が未実装の場合は追加する
  - タスク 1 の完了が前提（`AuthorAssociation` 型の依存）
  - _Requirements: 2.1, 2.3, 2.4_

- [ ] 3.2 (P) ReviewComment に author_association フィールドを追加する
  - `ReviewComment` 構造体に `author_association: AuthorAssociation` フィールドを追加する
  - GraphQL クエリから取得した文字列を `AuthorAssociation` に変換するロジックを追加する
  - 未知の値は `AuthorAssociation::None` として扱う（安全側）
  - タスク 1 の完了が前提（`AuthorAssociation` 型の依存）
  - _Requirements: 3.1_

- [ ] 4. REST クライアントに Timeline API と Permission API を実装する
  - タスク 3.1 の完了が前提（ポートインターフェースの依存）

- [ ] 4.1 Timeline API でラベル付与者のログインを取得する機能を実装する
  - `GET /repos/{owner}/{repo}/issues/{issue_number}/timeline` を呼び出す実装を追加する
  - レスポンスから `event == "labeled"` かつ `label.name == label_name` のイベントを逆順で検索する
  - 最新の labeled イベントの `actor.login` を返す
  - API 呼び出し失敗時はエラーを伝播する
  - _Requirements: 2.1_

- [ ] 4.2 Permission API でユーザーの association を取得する機能を実装する
  - `GET /repos/{owner}/{repo}/collaborators/{username}/permission` を呼び出す実装を追加する
  - `permission` フィールドを `RepositoryPermission` enum にマッピングする
  - 404 レスポンスは `RepositoryPermission::Read`（NONE 相当）として扱う
  - `RepositoryPermission::to_author_association()` で `AuthorAssociation` に変換する
  - _Requirements: 2.1_

- [ ] 5. (P) GraphQL クライアントに authorAssociation フィールドの取得を追加する
  - `list_unresolved_threads` の GraphQL クエリの `comments` ノードに `authorAssociation` フィールドを追加する
  - GraphQL レスポンスを `ReviewComment.author_association` にデシリアライズするロジックを更新する
  - タスク 3.2 の完了が前提。タスク 4 とは別ファイル（graphql_client.rs）のため並列実行可
  - _Requirements: 3.1_

- [ ] 6. association_guard モジュールを実装する
  - タスク 3.1 および タスク 4 の完了が前提

- [ ] 6.1 ラベル付与者の association チェックロジックを実装する
  - `AssociationCheckResult`（Trusted / Rejected）を定義する
  - `check_label_actor` 関数を実装する（`GitHubClient` と `TrustedAssociations` を受け取る）
  - `TrustedAssociations::All` の場合は API 呼び出しなしで即座に `Trusted` を返す
  - Timeline API で actor ログインを取得し、Permission API で association を判定する
  - チェック結果（許可/拒否）を `tracing::info!` でログ出力する
  - _Requirements: 2.1, 2.2, 2.5, 2.6_

- [ ] 6.2 権限不足時のラベル削除とコメント通知を実装する
  - `Rejected` の場合に `remove_label` を呼び出してラベルを削除する
  - ラベル削除後に `add_issue_comment` で拒否理由と信頼できる association レベルを通知するコメントを投稿する
  - コメント本文は設定されている `trusted_associations` の値を含む（ユーザーへのガイダンス）
  - _Requirements: 2.3, 2.4_

- [ ] 7. ポーリング処理に association チェックを組み込む
  - `step1_issue_polling` 内で新規 Issue 検出後に `check_label_actor` を呼び出す
  - `Trusted` の場合のみ `Event::IssueDetected` を push する
  - `Rejected` の場合はログ出力し、当該 Issue はスキップする
  - API エラー時はログ出力しワークフローを続行しない（安全側）
  - タスク 2、6 の完了が前提
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_

- [ ] 8. (P) PR レビューコメントのフィルタリングを実装する
  - `write_review_threads_input` に `trusted_associations: &TrustedAssociations` パラメータを追加する
  - `TrustedAssociations::All` の場合は全コメントを含める
  - `TrustedAssociations::Specific` の場合は `author_association` がリストにあるコメントのみを含める
  - 除外したコメント数と author の association レベルを `tracing::info!` でログ出力する
  - 呼び出し元（TransitionUseCase 等）に `config.trusted_associations` を渡す変更を加える
  - タスク 2、タスク 3.2、タスク 5 の完了が前提。タスク 6/7 とは別コードエリアのため並列実行可
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 9. セキュリティドキュメントと設定例を整備する

- [ ] 9.1 (P) SECURITY.md を作成する
  - プロンプトインジェクションリスクの説明を記載する
  - `trusted_associations` 設定の仕組みと推奨値を説明する
  - `"all"` 設定の使用条件（プライベートリポジトリ推奨）を明記する
  - 他のタスクと独立して実施可能
  - _Requirements: 4.1_

- [ ] 9.2 (P) README に Fork ワークフローと設定例を追記する
  - 外部コントリビューターが fork 上で Cupola を使い upstream へ PR を出す手順を記載する
  - `cupola.toml` に `trusted_associations` の設定例（デフォルト・`"all"` 設定）を追記する
  - 他のタスクと独立して実施可能
  - _Requirements: 4.2, 4.3_

- [ ] 10. ユニットテストと統合テストを実装する

- [ ] 10.1 ドメイン型と association_guard のユニットテストを実装する
  - `TrustedAssociations::is_trusted` の全バリアント組み合わせをテストする（All / Specific・各 AuthorAssociation）
  - `AuthorAssociation::from_str` の有効値・無効値・大文字小文字変換をテストする
  - `check_label_actor` のモック `GitHubClient` を使用した Trusted / Rejected 両ケースをテストする
  - `write_review_threads_input` の信頼済み/信頼外コメント混在ケースをテストする
  - _Requirements: 1.3, 1.4, 1.5, 2.1, 2.2, 2.5, 2.6, 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 10.2 統合テストを実装する
  - `step1_issue_polling` の association チェックを含む end-to-end フロー（mock adapter）をテストする
  - `CupolaToml` の `trusted_associations` 変換（`"all"` / リスト / 省略 / 無効値）をテストする
  - _Requirements: 1.1, 1.2, 1.3, 1.5, 2.1, 2.2, 2.3, 2.4_
