# i18n-github-comments サマリー

## Feature
`rust-i18n` クレートを導入し、GitHub Issue コメント 5 件 + フォールバック文字列 1 件を `cupola.toml` の `language` 設定に従って多言語（en / ja）出力する。

## 要件サマリ
1. `Cargo.toml` に `rust-i18n = "3"` を追加、`src/lib.rs` に `rust_i18n::i18n!("locales", fallback = "en")` を配置。
2. `locales/en.yml` / `locales/ja.yml` を作成し `issue_comment.{design_starting, implementation_starting, all_completed, cleanup_done, retry_exhausted, unknown_error}` を定義。`retry_exhausted` は `%{count}` / `%{error}` パラメータ補間をサポート。
3. `PollingUseCase` と `TransitionUseCase` の 5 + 1 箇所を `t!(key, locale = &self.config.language)` 呼び出しに置換。
4. 未知 locale は `fallback = "en"` で英語に落とす。
5. 既存テスト・Clippy を通す。

## アーキテクチャ決定
- **per-call locale** を採用し `rust_i18n::set_locale()` のようなグローバル state 変更は避ける。tokio 非同期並行環境での並行セッション干渉リスクを回避し、Clean Architecture の副作用局所化原則に合致。
- `locales/` ディレクトリはクレートルート (`Cargo.toml` 同階層) に配置（`i18n!` マクロのデフォルト解決）。
- `Config.language` は既存 `domain` フィールドをそのまま流用しドメイン変更なし。
- `t!()` の戻り値が `String` なため `unwrap_or(&str)` との型不整合が発生する箇所は、`let unknown = t!(...);` で事前バインドするパターンで解決。
- `FIXING_SCHEMA` との整合を保つため `ReviewComments` 非含有時も仕様上 `threads` は必須維持。

## コンポーネント
- `Cargo.toml`、`src/lib.rs`
- `locales/en.yml`、`locales/ja.yml`
- `src/application/polling_use_case.rs` (`design_starting`)
- `src/application/transition_use_case.rs` (`implementation_starting`, `all_completed`, `cleanup_done`, `retry_exhausted`, `unknown_error`)

## 主要インターフェース
- `t!("issue_comment.<key>", locale = &self.config.language, count = ..., error = ...)`

## 学び/トレードオフ
- グローバル `set_locale` はスレッド安全でないためマルチセッション環境では不適合。
- `format!()` による手動フォーマットを廃止し `t!()` の named parameter 補間に統一。
- ja.yml の文字列を現行ハードコードと一致させれば既存統合テスト（`msg.contains("リトライ上限")`）は変更不要で通過可能。
