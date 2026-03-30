# Implementation Plan

- [ ] 1. gh-token クレートを依存関係に追加する
  - `Cargo.toml` の `[dependencies]` セクションに `gh-token = "0.1"` を追加する
  - `cargo build` を実行して依存解決に問題がないことを確認する
  - `serde_yaml` 0.9 との競合がないことを `cargo tree` で確認する
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 2. resolve_github_token() を gh_token::get() に置き換える

- [ ] 2.1 (P) github_rest_client.rs から resolve_github_token() を削除する
  - `resolve_github_token()` 関数の実装（L141-172）を削除する
  - テストモジュール内の `resolve_token_from_env` テストを削除する
  - テストモジュールが空になる場合は `#[cfg(test)] mod tests` ブロックごと削除する
  - _Requirements: 2.2, 5.2_

- [ ] 2.2 (P) bootstrap/app.rs の呼び出し箇所を gh_token::get() に変更する
  - `resolve_github_token` の `use` 文を削除する
  - `let token = resolve_github_token()?;` を `let token = gh_token::get()?;` に変更する
  - `cargo build` を実行してコンパイルエラーがないことを確認する
  - _Requirements: 2.1, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4, 3.5, 4.1, 4.2, 4.3_

- [ ] 3. ビルドと既存テストの通過を確認する
  - `cargo fmt` を実行してコードフォーマットを統一する
  - `cargo clippy -- -D warnings` を実行して lint エラーがないことを確認する
  - `cargo test` を実行してすべてのテストが通ることを確認する
  - _Requirements: 1.2, 5.1, 5.3_
