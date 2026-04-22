# Implementation Plan

- [ ] 1. Cargo.toml の lint 設定に unwrap_used を追加する
  - `Cargo.toml` の `[lints.clippy]` セクションで `expect_used = "deny"` の直後に `unwrap_used = "deny"` を追加する
  - 既存の `all = { level = "warn", priority = -1 }` と `expect_used = "deny"` は変更しない
  - `devbox run clippy` を実行し、本番コード (`--lib`) でエラー 0 件を確認する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 2. lib.rs のテスト用 allow 属性に unwrap_used を追加する
  - `src/lib.rs` 先頭の `#![cfg_attr(test, allow(clippy::expect_used))]` を `#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]` に変更する
  - `devbox run test` を実行し、全テストがエラーなく通過することを確認する
  - `devbox run clippy` で `--all-targets` 含めてエラーが出ないことを確認する
  - _Requirements: 2.1, 2.2, 2.3, 2.4_
