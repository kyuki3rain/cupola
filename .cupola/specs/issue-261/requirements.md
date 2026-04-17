# Requirements Document

## Introduction

`src/adapter/outbound/github_rest_client.rs` の複数箇所で HTTP エラーをすべて `anyhow!()` に畳んでいるため、呼び出し側が 429 (rate limit) / 401・403 (auth) / 5xx (server) を区別できない。本機能では `thiserror` を用いた型付きエラー enum `GitHubApiError` をアダプター層に導入し、エラー種別の識別と適切なリトライ/停止判断を可能にする。

## Requirements

### Requirement 1: GitHubApiError enum の定義

**Objective:** アダプター開発者として、GitHub API の HTTP エラーを型付き enum で表現したい。そうすることで、エラー種別に応じた処理分岐をコンパイル時に保証できる。

#### Acceptance Criteria

1. The GitHubApiError shall `RateLimit { retry_after: Option<Duration> }` バリアントを持ち、429 レスポンスを表現する。
2. The GitHubApiError shall `Unauthorized` バリアントを持ち、401 レスポンスを表現する。
3. The GitHubApiError shall `Forbidden(String)` バリアントを持ち、403 レスポンスをメッセージ付きで表現する。
4. The GitHubApiError shall `ServerError { status: StatusCode }` バリアントを持ち、5xx レスポンスを表現する。
5. The GitHubApiError shall `NotFound { resource: String }` バリアントを持ち、404 レスポンスをリソース名付きで表現する。
6. The GitHubApiError shall `Other(#[from] anyhow::Error)` バリアントを持ち、上記以外のエラーを anyhow::Error でラップする。
7. The GitHubApiError shall `thiserror::Error` を derive し、各バリアントに意味のある `#[error(...)]` メッセージを持つ。
8. The GitHubApiError shall `anyhow::Error` に `From<GitHubApiError>` 経由で変換可能であり、既存の `?` 演算子による伝播を維持する。

### Requirement 2: HTTP レスポンスからの GitHubApiError 変換

**Objective:** アダプター開発者として、HTTP ステータスコードを GitHubApiError の適切なバリアントに変換したい。そうすることで、各 API メソッド内で重複なくエラー分類できる。

#### Acceptance Criteria

1. When HTTP ステータスが 429 のとき, the GitHubApiError converter shall `Retry-After` ヘッダーを秒数として解析し `RateLimit { retry_after: Some(duration) }` を返す。
2. When HTTP ステータスが 429 かつ `Retry-After` ヘッダーが存在しないとき, the GitHubApiError converter shall `RateLimit { retry_after: None }` を返す。
3. When HTTP ステータスが 401 のとき, the GitHubApiError converter shall `Unauthorized` を返す。
4. When HTTP ステータスが 403 のとき, the GitHubApiError converter shall レスポンスボディを含む `Forbidden(body)` を返す。
5. When HTTP ステータスが 5xx のとき, the GitHubApiError converter shall `ServerError { status }` を返す。
6. When HTTP ステータスが 404 のとき, the GitHubApiError converter shall `NotFound { resource }` を返す。
7. The github_rest_client.rs shall 既存の `anyhow!()` による HTTP エラー生成を `GitHubApiError` に置き換え、5 箇所すべてのエラーサイトを更新する。

### Requirement 3: アダプター内リトライロジック

**Objective:** アダプター開発者として、エラー種別に応じたリトライ判断をアダプター層で行いたい。そうすることで、アプリケーション層に transient/permanent の判断ロジックが漏れ出さない。

#### Acceptance Criteria

1. When GitHubApiError::RateLimit を受け取ったとき, the github_client_impl.rs shall `retry_after` の値だけ待機してから同一リクエストをリトライする。
2. When GitHubApiError::RateLimit で `retry_after` が None のとき, the github_client_impl.rs shall デフォルト待機時間 (60秒) を使用してリトライする。
3. When GitHubApiError::ServerError を受け取ったとき, the github_client_impl.rs shall 指数バックオフ (初期 1 秒、最大 3 回) でリトライする。
4. When GitHubApiError::Unauthorized または GitHubApiError::Forbidden を受け取ったとき, the github_client_impl.rs shall リトライせずに即座にエラーを伝播する。
5. The retry_policy.rs shall リトライ回数と待機時間を計算するヘルパーとして利用可能であり、アダプター層から呼び出せる。
6. The github_client_impl.rs shall リトライ試行のたびに `tracing::warn!` でエラー種別・試行回数・次回待機時間をログ出力する。

### Requirement 4: 既存インターフェースとの後方互換性

**Objective:** アプリケーション開発者として、ポートトレイト `GitHubClient` のインターフェースを変更せずに恩恵を受けたい。そうすることで、既存のユースケースコードを変更せずに済む。

#### Acceptance Criteria

1. The `GitHubClient` port trait shall 戻り値の型 `anyhow::Result<T>` を維持し、署名を変更しない。
2. The github_client_impl.rs shall リトライ後に残ったエラーを `anyhow::Error` に変換してポートトレイトの戻り値型に適合させる。
3. The github_rest_client.rs shall 404 を特別扱いする既存の idempotent 処理 (`fetch_user_permission`・`remove_label`) を維持する。
4. The existing integration tests shall 変更なしでコンパイル・パスする。
