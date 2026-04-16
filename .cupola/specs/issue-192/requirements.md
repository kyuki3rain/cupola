# 要件定義書

## はじめに

本仕様は `src/application/polling/execute.rs` に存在する2つのバグを修正するものです。

1. `PostCiFixLimitComment` エフェクトがコメント本文を英語でハードコードしており、`config.language` を参照していない
2. `RejectUntrustedReadyIssue` エフェクトがコメント本文を英語でハードコードしており、コメント投稿エラーを無声に握り潰している

両修正ともに既存の `rust_i18n` パターンおよびベストエフォートエラーログ規約に準拠させることが目的です。

## 要件

### Requirement 1: PostCiFixLimitComment の i18n 対応

**目的:** 開発者として、CI 修正上限コメントを `config.language` に基づいて正しい言語で投稿したい。そうすることで、ユーザーの言語設定が一貫して適用される。

#### 受け入れ基準

1. The system shall have `issue_comment.ci_fix_limit` key in `locales/en.yml` containing an English message template with `%{max_cycles}` placeholder.
2. The system shall have `issue_comment.ci_fix_limit` key in `locales/ja.yml` containing a Japanese message template with `%{max_cycles}` placeholder.
3. When `Effect::PostCiFixLimitComment` is executed, the system shall generate the comment body using `rust_i18n::t!("issue_comment.ci_fix_limit", locale = lang, max_cycles = config.max_ci_fix_cycles)`.
4. When `Effect::PostCiFixLimitComment` is executed, the system shall substitute `%{max_cycles}` with the actual value of `config.max_ci_fix_cycles` in the rendered message.
5. When `Effect::PostCiFixLimitComment` is executed and `config.language` is set to `"ja"`, the system shall post a Japanese message to GitHub.
6. When `Effect::PostCiFixLimitComment` is executed and `config.language` is set to `"en"`, the system shall post an English message to GitHub.

### Requirement 2: RejectUntrustedReadyIssue の i18n 対応

**目的:** 開発者として、信頼されていないアクターによるラベル付けを拒否するコメントを `config.language` に基づいて正しい言語で投稿したい。そうすることで、ユーザーの言語設定が一貫して適用される。

#### 受け入れ基準

1. The system shall have `issue_comment.reject_untrusted` key in `locales/en.yml` containing an English rejection message.
2. The system shall have `issue_comment.reject_untrusted` key in `locales/ja.yml` containing a Japanese rejection message.
3. When `Effect::RejectUntrustedReadyIssue` is executed and the label is removed successfully, the system shall generate the comment body using `rust_i18n::t!("issue_comment.reject_untrusted", locale = lang)`.

### Requirement 3: RejectUntrustedReadyIssue のエラーログ修正

**目的:** 開発者として、コメント投稿失敗時にベストエフォートログ規約に従った警告ログが出力されることを期待する。そうすることで、障害時のトレーサビリティが確保される。

#### 受け入れ基準

1. When `Effect::RejectUntrustedReadyIssue` is executed, the label is removed successfully, but `github.comment_on_issue()` returns an error, the system shall emit a `tracing::warn!()` log containing `issue_number` and `error` fields.
2. If `github.comment_on_issue()` returns an error for `RejectUntrustedReadyIssue`, the system shall not silently discard the error (i.e., `let _ = ...` pattern must not be used).
3. The system shall follow the best-effort convention established in `execute_effects()` — failures are logged but do not abort the effect chain.

### Requirement 4: 既存テストの維持とテストカバレッジ

**目的:** 開発者として、変更後も既存テストが通過し、新しい振る舞いに対してユニットテストが追加されることを期待する。そうすることで、リグレッションを防止できる。

#### 受け入れ基準

1. The system shall pass all existing tests in `src/application/polling/execute.rs` after the changes.
2. When `Effect::PostCiFixLimitComment` is executed with a mock GitHub client, the system shall post exactly one comment whose content matches the expected i18n template output with `max_cycles` substituted.
3. When `Effect::RejectUntrustedReadyIssue` is executed and `github.comment_on_issue()` returns an error, the system shall call `tracing::warn!()` instead of silently dropping the error.
4. The system shall verify that `%{max_cycles}` is correctly interpolated in both English and Japanese locale messages.
