# 実装計画

- [x] 1. ドメインモデルと DB スキーマの拡張
- [x] 1.1 (P) `Issue` エンティティと `MetadataUpdates` に `body_hash` フィールドを追加する
  - `Issue` 構造体に `body_hash: Option<String>` フィールドを末尾に追加する
  - `Issue::new` コンストラクタで `body_hash: None` を設定する
  - `Issue` 構造体リテラルを使う既存テスト（`issue_has_correct_fields` 等）に `body_hash: None` を追加してコンパイルエラーを解消する
  - `MetadataUpdates` 構造体に `body_hash: Option<Option<String>>` フィールドを追加する
  - `MetadataUpdates::apply_to` で `body_hash` の適用ロジック（`Some(Some(h))` でセット、`Some(None)` でクリア）を追加する
  - `MetadataUpdates` の既定値・フィールド保持・更新適用の検証を見直し、`body_hash` の設定・クリア・未変更が正しく扱われることを確認する
  - _Requirements: 1.2, 1.3_

- [x] 1.2 (P) SQLite スキーマとリポジトリに `body_hash` サポートを追加する
  - `sqlite_connection.rs` の `init_schema` に `Self::run_add_column_migration(&conn, "body_hash TEXT")?;` を追加する（`last_pr_review_submitted_at` マイグレーションの直後）
  - `sqlite_issue_repository.rs` の全 SELECT クエリ末尾に `body_hash` 列を追加する（列インデックス 12）
  - `row_to_issue` で `body_hash: row.get(12)?` を追加して `Issue` 構造体に設定する
  - `update_state_and_metadata` で `body_hash: Option<Option<String>>` の 3 ケース（`Some(Some(hash))`・`Some(None)` → Null・`None` → スキップ）を動的 SQL パターンで処理する
  - `body_hash` ラウンドトリップテスト（save → find_by_id で正しく取得できること）を追加する
  - マイグレーション冪等性テスト（既存 DB に列が追加されること、2 回実行しても失敗しないこと）を追加する
  - _Requirements: 1.1, 1.4_

- [x] 1.3 (P) `persist_decision` の「変更なし」早期リターン条件を更新する
  - `application/polling/persist.rs` の `persist_decision` にある早期リターン条件に `&& updates.body_hash.is_none()` を追加する
  - `body_hash` のみが設定された `MetadataUpdates` で DB 書き込みがスキップされないことをユニットテストで検証する
  - _Requirements: 1.3_

- [x] 2. SHA-256 ユーティリティと `BodyTamperedError` の追加
- [x] 2.1 (P) `sha2` クレートを追加し `sha256_hex` ユーティリティ関数を実装する
  - `Cargo.toml` の `[dependencies]` に `sha2 = "0.10"` を追加する
  - `application/polling/execute.rs` に `fn sha256_hex(text: &str) -> String` を実装する（`sha2::Sha256::digest(text.as_bytes())` → `format!("{:x}", result)`）
  - 同一入力に対して同一出力を返すことをユニットテストで検証する
  - 空文字列に対して既知の SHA-256 値 (`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`) を返すことを検証する
  - _Requirements: 2.1, 3.1_

- [x] 2.2 (P) `BodyTamperedError` エラー型を定義する
  - `application/polling/execute.rs` に `BodyTamperedError { issue_number: u64 }` を `thiserror::Error` derive で定義する
  - Display メッセージ（`"issue #{issue_number} body was modified after agent:ready approval"`）をユニットテストで検証する
  - _Requirements: 3.2_

- [x] 3. SpawnInit 時のハッシュ計算・保存
- [x] 3.1 `prepare_init_handle` をハッシュを計算して返すよう変更する
  - 戻り値の型を `Result<(JoinHandle<anyhow::Result<String>>, String)>` に変更する（第二要素が body_hash hex 文字列）
  - `github.get_issue()` で取得した `detail.body` に対して `sha256_hex` を呼び出す
  - 既存の `issue_body` 変数は引き続き spawn タスクへの move キャプチャで使用する
  - _Requirements: 2.1_

- [x] 3.2 `spawn_init_task` でハッシュを保存してインメモリ `Issue` を更新する
  - `_issue_repo: &I` を `issue_repo: &I` に変更する（アンダースコアを除去）
  - `issue: &Issue` を `issue: &mut Issue` に変更する
  - `prepare_init_handle` の戻り値から `(handle, body_hash)` を分解して受け取る
  - `issue_repo.update_state_and_metadata(issue.id, MetadataUpdates { body_hash: Some(Some(body_hash.clone())), ..Default::default() })` でハッシュを DB に保存する
  - 保存後に `issue.body_hash = Some(body_hash)` でインメモリを更新する
  - 保存失敗時は `init_mgr.release_claim(issue.id)` してエラーを伝播する
  - `execute_one` の `Effect::SpawnInit` ブランチで `issue` を `&mut` として渡すよう確認・修正する
  - SpawnInit 後に `body_hash` が DB に保存されること（ラウンドトリップ）を統合テストで検証する
  - _Requirements: 2.1, 2.2_

- [x] 4. SpawnProcess 前のハッシュ検証と改変応答
- [x] 4.1 `prepare_process_spawn` にハッシュ比較ロジックを追加する
  - 既存の `let detail = github.get_issue(...).await?;` の直後に以下を追加する:
    - `let current_hash = sha256_hex(&detail.body);`
    - `issue.body_hash` が `Some(saved)` かつ `saved != &current_hash` の場合、`BodyTamperedError { issue_number: issue.github_issue_number }` を `anyhow::Error` にラップして返す
    - `body_hash` が `None` の場合はスキップして続行する
  - _Requirements: 3.1, 3.2, 3.3_

- [x] 4.2 `execute_one` の `Effect::SpawnProcess` ブランチに改変応答を実装する
  - `spawn_process(...)` の戻り値を直接 `?` で伝播させる代わりに変数に受け取る
  - `err.downcast_ref::<BodyTamperedError>().is_some()` で改変検知エラーか判定する
  - 改変検知時にヘルパー関数（または inline）で以下を順番に実行する:
    1. `issue_repo.update_state_and_metadata(issue.id, MetadataUpdates { state: Some(State::Cancelled), ..Default::default() })` を呼び出し、失敗時はエラーを伝播する
    2. `github.remove_label(n, "agent:ready")` を呼び出し、失敗時は `warn!` ログで続行する（best-effort）
    3. `github.comment_on_issue(n, &tamper_message)` を呼び出し、失敗時は `warn!` ログで続行する（best-effort）
    4. `issue.state = State::Cancelled` でインメモリを更新する
    5. `warn!` トレースイベントを発火する（`issue_number` フィールドを含む）
    6. `return Ok(())` で改変応答を終了する
  - 通常のエラー（`BodyTamperedError` 以外）は従来通り `?` で伝播する
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 4.3 (P) ハッシュ比較と改変応答の統合テストを追加する
  - Mock `GitHubClient` と Mock `IssueRepository` を使用した application 層テストを作成する
  - 改変なし（hash 一致）: 通常の spawn が成功するケースを検証する
  - 改変あり（hash 不一致）: `Cancelled` 遷移 + ラベル削除呼び出し + コメント投稿呼び出しが発生するケースを検証する
  - `body_hash = None`: ハッシュ比較をスキップして spawn が成功するケースを検証する
  - SpawnInit 後に body_hash が正しく保存されること（`issue.body_hash` がインメモリで更新されていること）を検証する
  - _Requirements: 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 4.4_
