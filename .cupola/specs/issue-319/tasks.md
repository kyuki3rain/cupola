# 実装計画

- [ ] 1. secrecy crate を依存関係に追加する
  - `Cargo.toml` の `[target.'cfg(unix)'.dependencies]` セクションに `secrecy = "0.10"` を追加する
  - `devbox run check` でコンパイルが通ることを確認する
  - _Requirements: 1.1, 1.2_

- [ ] 2. アダプター層の token フィールドを SecretString に置換する

- [ ] 2.1 (P) OctocrabRestClient の token フィールドとコンストラクタを更新する
  - `use secrecy::{ExposeSecret, SecretString}` を import に追加する
  - `token: String` フィールドを `token: SecretString` に変更する
  - `new(token: String, ...)` のシグネチャを `new(token: SecretString, ...)` に変更する
  - `octocrab::builder().personal_token(token.clone())` を `octocrab::builder().personal_token(token.expose_secret().to_string())` に変更する
  - 4箇所の `format!("Bearer {}", self.token)` を `format!("Bearer {}", self.token.expose_secret())` に変更する（`get_job_logs`、`fetch_label_actor_login`、`fetch_user_permission`、`remove_label`）
  - `#[cfg(test)] fn new_for_test()` 内の `token: "test-token".to_string()` を `token: SecretString::new("test-token".into())` に変更する
  - _Requirements: 2.1, 2.3, 2.5_

- [ ] 2.2 (P) GraphQLClient の token フィールドとコンストラクタを更新する
  - `use secrecy::{ExposeSecret, SecretString}` を import に追加する
  - `token: String` フィールドを `token: SecretString` に変更する
  - `new(token: String, ...)` のシグネチャを `new(token: SecretString, ...)` に変更する
  - `execute_raw()` 内の `format!("bearer {}", self.token)` を `format!("bearer {}", self.token.expose_secret())` に変更する
  - _Requirements: 2.2, 2.4, 2.5_

- [ ] 3. bootstrap でのトークンラップを更新する
  - `bootstrap/app.rs` に `use secrecy::SecretString;` を追加する
  - `let token = gh_token::get()?;` を `let token = SecretString::new(gh_token::get()?.into());` に変更する（2箇所：run_daemon_child 相当と start_polling 相当）
  - `token.clone()` は `SecretString` の `Clone` 実装により変更不要であることを確認する
  - _Requirements: 3.1, 3.2_

- [ ] 4. Debug マスキングのテストを追加し品質確認する

- [ ] 4.1 SecretString の Debug マスキングを検証するテストを追加する
  - `github_rest_client.rs` の `#[cfg(test)] mod tests` 内に `secret_string_debug_does_not_expose_token` テストを追加する
  - `SecretString::new("actual-token".into())` の `format!("{:?}", ...)` 出力に `"actual-token"` が含まれないことをアサートする
  - _Requirements: 4.1, 4.2_

- [ ] 4.2 品質保証を確認する
  - `devbox run clippy` で `cargo clippy -- -D warnings` が通過することを確認する
  - `devbox run test` で全テストが通過することを確認する（`pagination_tests` モジュールを含む）
  - _Requirements: 5.1, 5.2, 5.3_
