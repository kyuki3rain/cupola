# 要件定義書

## はじめに

`rust-i18n` クレートを導入し、Cupola が GitHub に投稿するコメントを `cupola.toml` の `language` 設定に従って多言語出力できるようにする。現時点では英語（`en`）と日本語（`ja`）の2言語のみを実装するが、将来の言語追加が locale ファイルの追加のみで完結する i18n 構造を構築する。

対象となるメッセージは GitHub Issue コメント5件とフォールバック文字列1件（計6件）である。現在これらは日本語でハードコードされており、`cupola.toml` の `language` 設定が反映されていない。

---

## 要件

### Requirement 1: rust-i18n クレートの依存追加

**目的:** 開発者として、`rust-i18n` クレートを依存関係に追加することで、標準的な i18n 機構を利用したい。これにより独自の翻訳ロジックを実装せずに済む。

#### 受け入れ基準

1. The Cupola shall `rust-i18n = "3"` を `Cargo.toml` の `[dependencies]` セクションに追加する。
2. The Cupola shall `src/lib.rs`（またはクレートルート）に `rust_i18n::i18n!("locales", fallback = "en")` マクロ呼び出しを配置する。
3. When `cargo build` を実行したとき、the Cupola shall ビルドエラーなくコンパイルを完了する。

---

### Requirement 2: locale ファイルの作成と管理

**目的:** 開発者として、言語ごとの翻訳文字列を YAML ファイルで管理することで、将来の言語追加をファイル追加のみで完結させたい。

#### 受け入れ基準

1. The Cupola shall `locales/en.yml` を作成し、以下のキーすべてに英語の翻訳文字列を定義する：
   - `issue_comment.design_starting`
   - `issue_comment.implementation_starting`
   - `issue_comment.all_completed`
   - `issue_comment.cleanup_done`
   - `issue_comment.retry_exhausted`（`%{count}` と `%{error}` のパラメータを含む）
   - `issue_comment.unknown_error`
2. The Cupola shall `locales/ja.yml` を作成し、上記と同一のキーに日本語の翻訳文字列を定義する。
3. The Cupola shall `issue_comment.retry_exhausted` キーにおいて `%{count}` と `%{error}` の2つのパラメータ補間をサポートする。
4. Where 新たな言語の locale ファイルを `locales/<lang>.yml` として追加したとき、the Cupola shall アプリケーションコードを変更せずにその言語のメッセージを出力できる。

---

### Requirement 3: GitHub Issue コメントの多言語化

**目的:** 運用者として、`cupola.toml` の `language` 設定に従った言語で GitHub Issue コメントが投稿されることで、多言語チームにおけるユーザー体験を向上させたい。

#### 受け入れ基準

1. When `PollingUseCase` が設計開始コメントを投稿するとき、the Cupola shall `Config.language` を locale として `t!("issue_comment.design_starting", locale = ...)` で翻訳された文字列を使用する。
2. When `TransitionUseCase` が実装開始コメントを投稿するとき、the Cupola shall `Config.language` を locale として `t!("issue_comment.implementation_starting", locale = ...)` で翻訳された文字列を使用する。
3. When `TransitionUseCase` が全工程完了コメントを投稿するとき、the Cupola shall `Config.language` を locale として `t!("issue_comment.all_completed", locale = ...)` で翻訳された文字列を使用する。
4. When `TransitionUseCase` が cleanup 完了コメントを投稿するとき、the Cupola shall `Config.language` を locale として `t!("issue_comment.cleanup_done", locale = ...)` で翻訳された文字列を使用する。
5. When `TransitionUseCase` がリトライ上限到達コメントを投稿するとき、the Cupola shall `retry_count` と `error_message` をパラメータとして `t!("issue_comment.retry_exhausted", locale = ..., count = ..., error = ...)` で翻訳された文字列を使用する。
6. When `TransitionUseCase` がエラーメッセージの `unwrap_or` フォールバック文字列を使用するとき、the Cupola shall `t!("issue_comment.unknown_error", locale = ...)` で翻訳された文字列を使用する。
7. The Cupola shall グローバルな locale 状態を変更せず、`Config.language` を毎回 per-call パラメータとして `t!()` マクロに渡す。

---

### Requirement 4: 言語設定と未知 locale のフォールバック

**目的:** 運用者として、未対応言語が設定されている場合でもシステムが正常動作することで、設定ミスによる障害を防ぎたい。

#### 受け入れ基準

1. When `language = "en"` が設定されているとき、the Cupola shall `locales/en.yml` の英語文字列を GitHub Issue コメントとして投稿する。
2. When `language = "ja"` が設定されているとき、the Cupola shall `locales/ja.yml` の日本語文字列を GitHub Issue コメントとして投稿する。
3. If `language` に未対応の locale（例: `"zh"`、`"fr"`）が設定されているとき、the Cupola shall `fallback = "en"` の設定により英語文字列にフォールバックして投稿する。
4. The Cupola shall フォールバック発生時にエラーを発生させず、正常にメッセージを投稿する。

---

### Requirement 5: 既存テストの整合性維持

**目的:** 開発者として、i18n 化後も既存のテストが正しく動作することで、リグレッションを防ぎたい。

#### 受け入れ基準

1. When `cargo test` を実行したとき、the Cupola shall すべてのテストがエラーなく通過する。
2. When `transition_use_case.rs` または `polling_use_case.rs` のテストが GitHub Issue コメント文字列を検証しているとき、the Cupola shall テスト内の期待値を i18n 化後の正しい文字列（または `t!()` マクロ経由の結果）に合わせて更新する。
3. When `RUSTFLAGS=-D warnings cargo clippy --all-targets` を実行したとき、the Cupola shall すべての Clippy 警告がエラーなく通過する。
