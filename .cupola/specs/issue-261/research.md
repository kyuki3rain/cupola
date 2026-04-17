# Research & Design Decisions

---
**Purpose**: `GitHubApiError` 型付きエラー enum の導入に際した調査記録と設計判断の根拠。

---

## Summary

- **Feature**: issue-261 — GitHub REST クライアントの型付きエラー enum 導入
- **Discovery Scope**: Extension（既存アダプターへの拡張）
- **Key Findings**:
  - `github_rest_client.rs` に HTTP エラーを `anyhow!()` で一律処理する箇所が 5 つ存在する
  - アプリケーション層の `RetryPolicy` は DB 更新専用であり、HTTP リトライには未接続
  - ポートトレイト `GitHubClient` は `anyhow::Result<T>` を使用しており、今回の変更範囲はアダプター層に留める

## Research Log

### 既存エラーサイトの調査

- **Context**: `github_rest_client.rs` の HTTP エラー処理が `anyhow!()` で一律処理されており、429/403/5xx の区別ができない
- **Findings**:
  - `get_job_logs()`: `anyhow!("job logs API returned {status}: {body}")`
  - `fetch_label_actor_login()`: `anyhow!("timeline API returned {status}: {body}")` (+ ページネーション上限エラー)
  - `fetch_user_permission()`: `anyhow!("permission API returned {status}: {body}")` — 404 は `Ok(Read)` で idempotent
  - `remove_label()`: `anyhow!("remove label API returned {status}: {body}")` — 404 は `Ok(())` で idempotent
- **Implications**: 5 箇所すべてを `GitHubApiError` に置き換える必要がある。ただし 404 の特別処理は維持する。

### thiserror の採用

- **Context**: tech.md で `thiserror (domain/app) + anyhow (adapter/bootstrap)` が方針として明記されているが、今回はアダプター層に `thiserror` を使用する
- **Findings**:
  - `GitHubApiError` はアダプター内部での分類に使用し、ポートトレイト境界では `anyhow::Error` に変換する設計のため、clean architecture の境界は維持される
  - アダプター層でも `thiserror` を使うことで型安全な match が可能になり、内部リトライロジックの実装が明確になる
  - `From<GitHubApiError> for anyhow::Error` は `thiserror` + `std::error::Error` の derive によって自動生成される
- **Implications**: tech.md の「thiserror は domain/app 向け」という記述は strict ではなく、アダプター内部の型付きエラーにも問題なく採用できる

### RetryPolicy の利用範囲

- **Context**: `src/application/retry_policy.rs` に `RetryPolicy` が存在するが、DB 更新のリトライ専用
- **Findings**:
  - `RetryPolicy::evaluate(current_retry_count) -> RetryDecision` という単純なカウンターベースのインターフェース
  - 待機時間の計算ロジックは含まれていない
  - アダプター層から直接 `RetryPolicy` を使うと application → adapter の依存となり clean architecture 違反
- **Implications**: アダプター内でシンプルなリトライループを自前実装する。`RetryPolicy` は今回使用しない。

### Retry-After ヘッダーの仕様

- **Context**: GitHub API は 429 レスポンスで `Retry-After` ヘッダーを返す場合がある
- **Findings**:
  - `Retry-After` ヘッダーの値は秒数 (整数) として提供される
  - ヘッダーが存在しない場合はデフォルト待機時間を使用する (GitHub のドキュメントでは 60 秒を推奨)
  - `reqwest::Response::headers()` でヘッダーアクセス可能
- **Implications**: `RateLimit { retry_after: Option<Duration> }` として表現し、None 時は 60 秒デフォルトを使用する

### エラー変換ヘルパーの配置

- **Context**: 5 箇所で共通の HTTP → GitHubApiError 変換が必要
- **Findings**:
  - ステータスコードとレスポンスボディを引数にとる変換関数を 1 か所にまとめることで DRY を実現
  - `github_rest_client.rs` 内の private 関数として配置するか、専用モジュール `github_api_error.rs` を作るかの選択肢がある
- **Implications**: 今回は `github_api_error.rs` を新規作成し、`GitHubApiError` の定義と変換ロジックを集約する（単一責任原則）

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: アダプター内型付きエラー (採用) | `GitHubApiError` をアダプター層に定義し、ポート境界で anyhow に変換 | clean architecture を維持、ポートトレイト変更不要 | ポートトレイト越しに型情報が消える | #166 でポートレベルの型付きエラーに昇格可能 |
| B: ポートトレイトの型変更 | `GitHubClient` の戻り値を `Result<T, GitHubClientError>` に変更 | アプリケーション層から型付きエラーを扱える | ユースケースの全修正が必要、#166 の範囲 | #166 に委ねる |
| C: anyhow downcast | anyhow::Error として伝播し、呼び出し側で downcast | 最小変更 | 下流の全呼び出し元に downcast コードが必要で危脆弱 | アンチパターン |

## Design Decisions

### Decision: GitHubApiError の配置場所

- **Context**: `GitHubApiError` を既存の `github_rest_client.rs` に追加するか、専用ファイルを作るか
- **Alternatives Considered**:
  1. `github_rest_client.rs` 内に定義 — ファイルの肥大化
  2. `src/adapter/outbound/github_api_error.rs` 新規作成 — 単一責任
- **Selected Approach**: `github_api_error.rs` を新規作成し、`github_rest_client.rs` および `github_client_impl.rs` から参照
- **Rationale**: steering の "One file, one responsibility" 原則に従う
- **Trade-offs**: ファイル数が増えるが、変換ロジックの変更時の影響範囲が明確になる
- **Follow-up**: `mod.rs` に `pub mod github_api_error;` を追加し再エクスポートする

### Decision: リトライロジックの実装場所

- **Context**: リトライは `github_rest_client.rs`（個別メソッド）か `github_client_impl.rs`（ファサード）か
- **Alternatives Considered**:
  1. `github_rest_client.rs` 内で個別実装 — 各メソッドに重複
  2. `github_client_impl.rs` のファサードで一括処理 — DRY、集中管理
- **Selected Approach**: `github_client_impl.rs` でリトライロジックをラッパーとして実装
- **Rationale**: 5 つの API メソッドすべてに個別にリトライを書くより、ファサード層でポリシーを統一する方が保守性が高い
- **Trade-offs**: ファサードが複雑になるが、テストが 1 か所で済む

## Risks & Mitigations

- **Rate limit の無限リトライ**: retry_after が異常値の場合の上限未設定 → リトライ最大回数 (例: 3 回) を設ける
- **tokio::time::sleep による async ブロック**: リトライ待機は `tokio::time::sleep` を使用し、スレッドをブロックしない
- **既存テストの破損**: 404 の idempotent 処理が変わると既存テストが壊れる → 変換関数で 404 を早期リターンせず、各メソッドで明示的に分岐を維持する

## References

- GitHub REST API Rate Limiting — `Retry-After` ヘッダーの仕様
- `thiserror` crate docs — `#[from]` による `From` 自動実装
- `src/adapter/outbound/github_rest_client.rs` — 現状の 5 エラーサイト調査
- Issue #166 — レイヤーごとの型付きエラー設計（上位方針）
