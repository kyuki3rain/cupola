# 実装計画

- [ ] 1. ロケールファイルに i18n キーを追加する
- [ ] 1.1 (P) `locales/en.yml` に `ci_fix_limit` キーを追加する
  - `issue_comment:` セクションに `ci_fix_limit` キーを追加
  - メッセージに `%{max_cycles}` プレースホルダーを含める
  - _Requirements: 1.1_

- [ ] 1.2 (P) `locales/ja.yml` に `ci_fix_limit` キーを追加する
  - `issue_comment:` セクションに `ci_fix_limit` キーを追加
  - 日本語メッセージに `%{max_cycles}` プレースホルダーを含める
  - _Requirements: 1.2_

- [ ] 1.3 (P) `locales/en.yml` に `reject_untrusted` キーを追加する
  - `issue_comment:` セクションに `reject_untrusted` キーを追加（プレースホルダーなし）
  - _Requirements: 2.1_

- [ ] 1.4 (P) `locales/ja.yml` に `reject_untrusted` キーを追加する
  - `issue_comment:` セクションに `reject_untrusted` キーを追加（プレースホルダーなし）
  - _Requirements: 2.2_

- [ ] 2. `execute.rs` の2つのエフェクトハンドラを修正する
- [ ] 2.1 `Effect::PostCiFixLimitComment` を `rust_i18n::t!()` に移行する
  - `format!()` によるハードコードメッセージを `rust_i18n::t!("issue_comment.ci_fix_limit", locale = lang, max_cycles = config.max_ci_fix_cycles)` に置き換える
  - タスク 1 完了後に着手する
  - _Requirements: 1.3, 1.4, 1.5, 1.6_

- [ ] 2.2 `Effect::RejectUntrustedReadyIssue` を `rust_i18n::t!()` に移行し、エラーログを修正する
  - ハードコードメッセージを `rust_i18n::t!("issue_comment.reject_untrusted", locale = lang)` に置き換える
  - `let _ = github.comment_on_issue(...)` を `if let Err(e) = github.comment_on_issue(n, &msg).await { tracing::warn!(issue_number = n, error = %e, "...") }` に置き換える
  - タスク 1 完了後に着手する
  - _Requirements: 2.3, 3.1, 3.2, 3.3_

- [ ] 3. ユニットテストを追加する
- [ ] 3.1 (P) `PostCiFixLimitComment` の i18n 動作をテストする
  - 日本語ロケール設定時に日本語メッセージが投稿されることを検証
  - `%{max_cycles}` が `config.max_ci_fix_cycles` の値に正しく置換されることを検証
  - タスク 2.1 完了後に着手する
  - _Requirements: 4.2, 4.4_

- [ ] 3.2 (P) `RejectUntrustedReadyIssue` のエラーログ動作をテストする
  - `github.comment_on_issue()` がエラーを返した場合に `let _` で握り潰されないことを検証
  - モックで `comment_on_issue` をエラーにした場合、`tracing::warn!` が呼ばれることを確認
  - タスク 2.2 完了後に着手する
  - _Requirements: 4.3_

- [ ] 4. 品質チェックを実施する
  - `devbox run clippy` を実行してエラーがないことを確認
  - `devbox run test` を実行して全テストがパスすることを確認
  - `devbox run fmt-check` を実行してフォーマットが正しいことを確認
  - _Requirements: 4.1_
