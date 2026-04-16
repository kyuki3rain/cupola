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

1. The system shall `locales/en.yml` に `issue_comment.ci_fix_limit` キーを持ち、`%{max_cycles}` プレースホルダーを含む英語メッセージテンプレートを定義する。
2. The system shall `locales/ja.yml` に `issue_comment.ci_fix_limit` キーを持ち、`%{max_cycles}` プレースホルダーを含む日本語メッセージテンプレートを定義する。
3. When `Effect::PostCiFixLimitComment` が実行されたとき、the system shall ロケール設定に応じた i18n 関数を用いてコメント本文を生成する。
4. When `Effect::PostCiFixLimitComment` が実行されたとき、the system shall レンダリング済みメッセージの `%{max_cycles}` を `config.max_ci_fix_cycles` の実際の値に置換する。
5. When `Effect::PostCiFixLimitComment` が実行され、`config.language` が `"ja"` に設定されているとき、the system shall 日本語メッセージを GitHub に投稿する。
6. When `Effect::PostCiFixLimitComment` が実行され、`config.language` が `"en"` に設定されているとき、the system shall 英語メッセージを GitHub に投稿する。

### Requirement 2: RejectUntrustedReadyIssue の i18n 対応

**目的:** 開発者として、信頼されていないアクターによるラベル付けを拒否するコメントを `config.language` に基づいて正しい言語で投稿したい。そうすることで、ユーザーの言語設定が一貫して適用される。

#### 受け入れ基準

1. The system shall `locales/en.yml` に `issue_comment.reject_untrusted` キーを持ち、英語の拒否メッセージを定義する。
2. The system shall `locales/ja.yml` に `issue_comment.reject_untrusted` キーを持ち、日本語の拒否メッセージを定義する。
3. When `Effect::RejectUntrustedReadyIssue` が実行されラベル削除が成功したとき、the system shall ロケール設定に応じた i18n 関数を用いてコメント本文を生成する。

### Requirement 3: RejectUntrustedReadyIssue のエラーログ修正

**目的:** 開発者として、コメント投稿失敗時にベストエフォートログ規約に従った警告ログが出力されることを期待する。そうすることで、障害時のトレーサビリティが確保される。

#### 受け入れ基準

1. When `Effect::RejectUntrustedReadyIssue` が実行され、ラベル削除に成功したが `github.comment_on_issue()` がエラーを返したとき、the system shall `issue_number` および `error` フィールドを含む警告ログを出力する。
2. If `RejectUntrustedReadyIssue` において `github.comment_on_issue()` がエラーを返した場合、the system shall エラーを無声に破棄しない（`let _ = ...` パターンを使用しない）。
3. The system shall `execute_effects()` で確立されたベストエフォート規約に従い、失敗はログに記録するがエフェクトチェーンを中断しない。

### Requirement 4: 既存テストの維持とテストカバレッジ

**目的:** 開発者として、変更後も既存テストが通過し、新しい振る舞いに対してユニットテストが追加されることを期待する。そうすることで、リグレッションを防止できる。

#### 受け入れ基準

1. The system shall 変更後も `src/application/polling/execute.rs` の既存テストをすべてパスする。
2. When `Effect::PostCiFixLimitComment` がモック GitHub クライアントで実行されたとき、the system shall `max_cycles` を置換した期待する i18n テンプレート出力と一致する内容のコメントをちょうど1件投稿する。
3. When `Effect::RejectUntrustedReadyIssue` が実行され `github.comment_on_issue()` がエラーを返したとき、the system shall エラーを無声に破棄する代わりに警告ログを出力する。
4. The system shall 英語および日本語ロケールのメッセージで `%{max_cycles}` が正しく補間されることを検証する。
