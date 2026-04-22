# リサーチ・設計決定記録

---
**機能**: issue-319 — GitHub token の SecretString ラップ
**調査スコープ**: Extension（既存アダプター層への型安全強化）

---

## サマリー

- **機能**: issue-319 — GitHub token の SecretString ラップ
- **調査スコープ**: Extension（既存アダプター層への型安全強化）
- **主な発見事項**:
  - `secrecy 0.10` は `Zeroize` ベースのメモリ消去と `Debug` 出力マスクを提供する
  - `SecretString` は `Clone` 実装があり、bootstrap での `token.clone()` パターンがそのまま動作する
  - `.expose_secret()` は `ExposeSecret` トレイトによる明示的取り出しで、grep による漏洩箇所特定が容易になる
  - 変更対象ファイルは `github_rest_client.rs`、`github_graphql_client.rs`、`bootstrap/app.rs`、`Cargo.toml` の4ファイルのみ
  - ドメイン層・アプリケーション層への変更はゼロ

## リサーチログ

### secrecy crate の API 確認

- **コンテキスト**: 導入する `secrecy` crate のバージョンと API の確認
- **調査源**: crates.io secrecy ドキュメント、GitHub kyuki3rain/cupola の既存 Cargo.toml
- **発見事項**:
  - `SecretString` は `Secret<String>` のエイリアス
  - `Debug` 実装: `Secret([REDACTED])` を出力（実際のトークン値は含まれない）
  - `Display` 実装なし（明示的に `.expose_secret()` が必要）
  - `Clone` 実装あり（bootstrap での `clone()` が可能）
  - `Zeroize` により、ドロップ時にメモリ上の値がゼロクリアされる
- **示唆**: bootstrap の `token.clone()` パターンは変更不要。`.expose_secret()` の呼び出し箇所が `grep expose_secret` で一覧できるため、セキュリティ監査が容易

### 既存コードの token フィールド使用箇所の調査

- **コンテキスト**: 変更が必要な箇所を特定するためのコード調査
- **調査源**: `github_rest_client.rs`、`github_graphql_client.rs`、`bootstrap/app.rs` の直接読み取り
- **発見事項**:
  - `OctocrabRestClient.token` の使用箇所（4メソッド）:
    - `get_job_logs()` — `format!("Bearer {}", self.token)` (line ~116)
    - `fetch_label_actor_login()` — `format!("Bearer {}", self.token)` (line ~300)
    - `fetch_user_permission()` — `format!("Bearer {}", self.token)` (line ~366)
    - `remove_label()` — `format!("Bearer {}", self.token)` (line ~412)
  - `OctocrabRestClient::new()` では `octocrab::builder().personal_token(token.clone())` に渡している
  - テスト用 `new_for_test()` では `token: "test-token".to_string()` フィールド初期化が必要
  - `GraphQLClient.token` の使用箇所（1メソッド）:
    - `execute_raw()` — `format!("bearer {}", self.token)` (line ~279)
  - `bootstrap/app.rs` では2箇所で同じパターン（line ~525, ~690）:
    ```
    let token = gh_token::get()?;
    let rest = OctocrabRestClient::new(token.clone(), ...)?;
    let graphql = GraphQLClient::new(token, ...);
    ```
- **示唆**: 変更箇所は限定的。すべてアダプター層とブートストラップ層に収まる

### `secrecy` の unix 依存セクションへの追加

- **コンテキスト**: Cargo.toml の依存関係セクション構成を確認
- **発見事項**: 既存の依存関係は `[target.'cfg(unix)'.dependencies]` セクションにまとめられている（`gh-token`、`nix` 等）。`secrecy` も同セクションに追加する
- **示唆**: `secrecy = "0.10"` を `[target.'cfg(unix)'.dependencies]` に追加する

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク / 制限 | 備考 |
|------------|------|------|---------------|------|
| secrecy crate | `SecretString` による型ラップ | 標準的・Zeroize 対応・`expose_secret()` で監査が容易 | 外部依存1件追加 | Issue の推奨案と一致 |
| 自前 newtype | `Display` なし newtype を手実装 | 依存追加なし | Zeroize 非対応・メンテナンスコスト大 | 不採用（Issue でも不採用と明記） |
| 現状維持 | `String` のまま放置 | 変更コストゼロ | 将来の `Debug` 追加・`format!()` 混入リスクを型で封じられない | 不採用 |

## 設計決定

### 決定: SecretString の適用範囲をアダプター層とブートストラップ層に限定

- **コンテキスト**: `SecretString` をどの層まで浸透させるか
- **検討した代替案**:
  1. ドメイン層・アプリケーション層まで `SecretString` を浸透させる
  2. アダプター層とブートストラップ層のみに適用する
- **選択したアプローチ**: アダプター層（`OctocrabRestClient`、`GraphQLClient`）とブートストラップ層に限定
- **根拠**: トークンはドメインロジックとは無関係な外部認証情報。アプリケーション層のポート定義（`GitHubClient` トレイト）はトークンを受け取らず、クライアント構築時のみ必要。ドメイン・アプリケーション層への変更ゼロでクリーンアーキテクチャを維持できる
- **トレードオフ**: ドメイン層まで引き込む方が型安全性は高まるが、現構造では不要な過剰設計になる。bootstrap で一度 `SecretString` に包めば `.expose_secret()` の呼び出しはアダプター内のヘッダー構築時のみに限定できる
- **フォローアップ**: 実装後に `grep expose_secret` で呼び出し箇所が特定できることを確認する

## リスクと軽減策

- **コンパイルエラーの連鎖**: `String` → `SecretString` の変更でビルドが壊れる可能性 → `devbox run check` を早期に実行して型エラーを一括確認
- **テストの修正漏れ**: `new_for_test()` 内の `"test-token".to_string()` が残ると型エラー → grep で全 `token: "test-token"` を確認
- **octocrab builder の引数型**: `personal_token()` は `String` を要求するため `.expose_secret().to_string()` が必要 → 設計時に明示

## 参考資料

- secrecy crate — `SecretString`、`ExposeSecret`、Zeroize ベースのメモリ消去機構
- Issue #321 — 同系統のセキュリティ強化（env 最小化）。env_clear 対応と同時進行すると相性が良い
