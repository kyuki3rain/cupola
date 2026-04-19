# 要件定義書

## はじめに

GitHub PAT（Personal Access Token）を `String` 型から `secrecy::SecretString` 型へ置換する防御的セキュリティリファクタリングの要件を定義する。現状は漏洩経路が存在しないが、将来的な `Debug` derive 追加や `format!()` への誤った混入によるトークン漏洩を型レベルで封じることを目的とする。対象ファイルはアダプター層の2クライアント（`OctocrabRestClient`、`GraphQLClient`）と `bootstrap/app.rs` に限定され、ドメイン層・アプリケーション層への変更はない。

## 要件

### Requirement 1: secrecy crate の依存関係追加

**目的**: Cupola 開発者として、`secrecy` crate を依存関係に追加することで、型レベルのトークン漏洩防止機構を利用できるようにしたい。

#### 受け入れ条件

1. The system shall declare `secrecy` (version 0.10 以降) as a dependency in `Cargo.toml`.
2. When the project is built, the system shall compile successfully with `SecretString` and `ExposeSecret` accessible from adapter modules.

### Requirement 2: アダプター層トークンフィールドの SecretString 置換

**目的**: Cupola 開発者として、`OctocrabRestClient` と `GraphQLClient` のトークンフィールドを `SecretString` 型に変更することで、将来的な誤ったデバッグ出力によるトークン漏洩を型レベルで防ぎたい。

#### 受け入れ条件

1. The system shall declare `OctocrabRestClient.token` as `SecretString` type (not `String`).
2. The system shall declare `GraphQLClient.token` as `SecretString` type (not `String`).
3. When constructing `OctocrabRestClient`, the system shall accept a `SecretString` as the `token` parameter.
4. When constructing `GraphQLClient`, the system shall accept a `SecretString` as the `token` parameter.
5. When building HTTP `Authorization` headers, the system shall call `.expose_secret()` to extract the token string.

### Requirement 3: bootstrap でのSecretString ラップ

**目的**: Cupola 開発者として、`gh_token::get()` で取得した生トークンを `SecretString` にラップしてからアダプターに渡すことで、アプリケーション起動時点からトークンを型安全に保護したい。

#### 受け入れ条件

1. When the daemon starts, the system shall wrap the raw token from `gh_token::get()` in `SecretString::new()` before passing it to adapter constructors.
2. The system shall pass the `SecretString` token (via `.clone()`) to both `OctocrabRestClient` and `GraphQLClient` constructors.

### Requirement 4: Debug 出力でのトークンマスキング検証

**目的**: Cupola 開発者として、`SecretString` の `Debug` 出力が実際のトークン値を含まないことをテストで検証することで、将来的な意図しないデバッグ出力によるトークン漏洩を防止したい。

#### 受け入れ条件

1. When a `SecretString` containing a token value is formatted with `{:?}`, the system shall NOT output the actual token value.
2. The system shall output masked content (e.g., `Secret([REDACTED])`) instead of the actual token value in Debug format.

### Requirement 5: 後方互換性と品質保証

**目的**: Cupola 開発者として、既存テストが引き続き動作し、かつ clippy 警告がゼロであることを確認することで、セキュリティ強化と品質維持を両立したい。

#### 受け入れ条件

1. When running existing tests that initialize clients with string token literals (e.g., `"test-token"`), the system shall compile and pass after updating those literals to `SecretString`.
2. The system shall pass `cargo clippy -- -D warnings` with zero warnings or errors.
3. The system shall pass all existing unit and integration tests without regression.
