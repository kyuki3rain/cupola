# Implementation Plan

- [ ] 1. GitHubApiError enum を定義する
- [ ] 1.1 `github_api_error.rs` を新規作成し、GitHubApiError enum を定義する
  - `src/adapter/outbound/github_api_error.rs` を新規作成する
  - `RateLimit { retry_after: Option<Duration> }`、`Unauthorized`、`Forbidden(String)`、`ServerError { status: StatusCode }`、`NotFound { resource: String }`、`Other(#[from] anyhow::Error)` の 6 バリアントを定義する
  - `thiserror::Error` を derive し、各バリアントに意味のある `#[error(...)]` メッセージを付与する
  - `src/adapter/outbound/mod.rs` に `pub mod github_api_error;` を追加して再エクスポートする
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7_

- [ ] 1.2 `classify_http_error` 変換関数を実装する
  - `github_api_error.rs` 内に `pub fn classify_http_error(status: StatusCode, body: String, retry_after: Option<Duration>, resource: &str) -> GitHubApiError` を実装する
  - 429 → `RateLimit { retry_after }`、401 → `Unauthorized`、403 → `Forbidden(body)`、5xx → `ServerError { status }`、404 → `NotFound { resource }` の分岐を実装する
  - `Retry-After` ヘッダーの値が数値でない場合または欠損の場合は `None` として扱う
  - `anyhow::Error` への `From` 変換が自動導出されることを確認する
  - _Requirements: 1.8, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_

- [ ] 2. `github_rest_client.rs` のエラーサイトを置き換える
- [ ] 2.1 5 箇所のエラーサイトを `classify_http_error` に置き換える
  - `get_job_logs`、`fetch_label_actor_login`（2 箇所）、`fetch_user_permission`、`remove_label` の各エラーサイトを更新する
  - `resp.headers()` から `Retry-After` ヘッダーを読み取り `classify_http_error` に渡す
  - `fetch_user_permission` と `remove_label` の 404 idempotent 早期リターン処理は `classify_http_error` 呼び出し前に維持する
  - ページネーション上限エラー (`fetch_label_actor_login`) は HTTP ステータスと無関係なため `anyhow!()` のままとし `GitHubApiError::Other` でラップしない
  - メソッドの公開シグネチャは `anyhow::Result<T>` のままとし、末尾に `.map_err(Into::into)` で変換する
  - _Requirements: 2.7, 4.3_

- [ ] 3. `github_client_impl.rs` にリトライロジックを追加する
- [ ] 3.1 汎用リトライヘルパー `with_retry` を実装する
  - `github_client_impl.rs` 内に `async fn with_retry<T, F, Fut>(op_name: &str, f: F) -> anyhow::Result<T>` を実装する
  - `GitHubApiError::RateLimit` の場合: `retry_after` または 60 秒デフォルトで `tokio::time::sleep` 後リトライ
  - `GitHubApiError::ServerError` の場合: 指数バックオフ (1s → 2s → 4s) でリトライ
  - `GitHubApiError::Unauthorized` / `Forbidden` の場合: リトライなしで即時 `anyhow::Error` に変換して返す
  - `GitHubApiError::NotFound` / `Other` の場合: リトライなしで即時伝播する
  - リトライ最大 3 回。枯渇時は最後のエラーを `anyhow::Error` に変換して返す
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 3.2 リトライログ出力を追加する
  - リトライ試行ごとに `tracing::warn!(op_name, attempt, wait_secs, %error)` を出力する
  - リトライ枯渇時に `tracing::error!(op_name, %error)` を出力する
  - _Requirements: 3.6_

- [ ] 3.3 `GitHubClientImpl` の各メソッドで `with_retry` を使用する
  - `GitHubClient` トレイトのメソッド実装を `with_retry` でラップする
  - idempotent でない操作 (例: `remove_label`) については、リトライが安全かどうか確認し、安全でない場合は `with_retry` を使わない
  - ポートトレイトの戻り値 `anyhow::Result<T>` は変更しない
  - _Requirements: 4.1, 4.2_

- [ ] 4. ユニットテストを追加する
- [ ] 4.1 (P) `classify_http_error` のユニットテストを追加する
  - 各ステータスコード (429、401、403、500、404、200以外のその他) に対して期待バリアントが返ることをテストする
  - `Retry-After` ヘッダーが存在する場合・しない場合の両方をテストする
  - `Retry-After` に不正値 (数値でない文字列) が含まれる場合に `None` になることをテストする
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_

- [ ] 4.2 (P) `with_retry` のユニットテストを追加する
  - Mock クロージャで `RateLimit` を n 回返した後に `Ok` を返し、リトライが成功することをテストする
  - `Unauthorized` / `Forbidden` が 1 回で即時返却されることをテストする
  - リトライ最大 3 回で枯渇し `Err` が返ることをテストする
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [ ] 5. 既存テストとの互換性を確認する
- [ ] 5.1 既存のテストが変更なしでコンパイル・パスすることを確認する
  - `devbox run test` を実行してすべてのテストが通ることを確認する
  - `devbox run clippy` を実行して警告がないことを確認する
  - _Requirements: 4.4_
