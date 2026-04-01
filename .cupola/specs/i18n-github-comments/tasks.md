# Implementation Plan

- [x] 1. rust-i18n のセットアップ

- [x] 1.1 rust-i18n クレートを依存関係に追加する
  - `Cargo.toml` の `[dependencies]` セクションに `rust-i18n = "3"` を追加する
  - `cargo build` を実行してビルドが通ることを確認する
  - _Requirements: 1.1, 1.3_

- [x] 1.2 クレートルートに翻訳マクロを初期化する
  - `src/lib.rs` に `rust_i18n::i18n!("locales", fallback = "en")` マクロ呼び出しを追加する
  - `use rust_i18n::t;` のインポートが各ユースケースで利用可能になることを確認する
  - `cargo build` でコンパイルエラーがないことを確認する
  - _Requirements: 1.2, 1.3_

- [x] 2. locale ファイルの作成

- [x] 2.1 (P) 英語翻訳ファイルを作成する
  - `locales/en.yml` を新規作成し、`issue_comment` キー配下に6件の翻訳文字列を定義する
  - `design_starting`、`implementation_starting`、`all_completed`、`cleanup_done`、`retry_exhausted`、`unknown_error` のすべてを英語で記述する
  - `retry_exhausted` には `%{count}` と `%{error}` の2つのパラメータ補間プレースホルダーを含める
  - _Requirements: 2.1, 2.3, 2.4_

- [x] 2.2 (P) 日本語翻訳ファイルを作成する
  - `locales/ja.yml` を新規作成し、`issue_comment` キー配下に6件の翻訳文字列を定義する
  - 既存のハードコード文字列と完全に一致する内容（`"設計を開始します"` 等）を使用する
  - `retry_exhausted` には `%{count}` と `%{error}` の2つのパラメータ補間プレースホルダーを含める
  - _Requirements: 2.2, 2.3, 2.4_

- [x] 3. (P) PollingUseCase の設計開始コメントを多言語化する
  - `polling_use_case.rs` の設計開始コメント投稿箇所で、ハードコード文字列を `t!("issue_comment.design_starting", locale = &self.config.language)` に置き換える
  - `t!()` の戻り値（`String`）を `&` で参照として `comment_on_issue` に渡す
  - タスク 1・2 の完了後に着手する（翻訳マクロと locale ファイルが必要）
  - _Requirements: 3.1, 3.7_

- [x] 4. (P) TransitionUseCase の GitHub コメントを多言語化する

- [x] 4.1 (P) 実装開始・全工程完了・cleanup 完了のコメントを多言語化する
  - `transition_use_case.rs` の3箇所のハードコード文字列を `t!()` マクロに置き換える
  - `implementation_starting`、`all_completed`、`cleanup_done` の各キーを対応するコメント投稿箇所で使用する
  - `t!()` の戻り値を参照として渡すパターンを一貫して適用する
  - タスク 1・2 の完了後に着手する
  - _Requirements: 3.2, 3.3, 3.4, 3.7_

- [x] 4.2 (P) リトライ上限メッセージとフォールバック文字列を多言語化する
  - `retry_exhausted` のメッセージを `t!()` の named parameter 補間（`count`、`error`）で生成するよう置き換える
  - `unwrap_or` の `"不明"` フォールバックを `unknown_error` キーで翻訳した文字列を変数に先にバインドしてから使用するパターンに変更する（`t!()` が `String` を返すため `&str` との型整合を考慮）
  - `format!()` による手動フォーマットを廃止し `t!()` の補間のみで完結させる
  - タスク 1・2 の完了後に着手する
  - _Requirements: 3.5, 3.6, 3.7_

- [x] 5. テストの更新と品質確認

- [x] 5.1 統合テストの翻訳対応を確認する
  - 既存の統合テストが日本語文字列を参照している箇所（`msg.contains("リトライ上限")` 等）が `ja.yml` の内容と一致することを確認する
  - `ja.yml` の `retry_exhausted` 文字列が既存のアサーションを満たすことを検証する
  - `cargo test` を実行してすべてのテストが通ることを確認する
  - _Requirements: 5.1, 5.2_

- [x] 5.2 cargo clippy で静的解析を通過させる
  - `RUSTFLAGS=-D warnings cargo clippy --all-targets` を実行して警告がゼロであることを確認する
  - `t!()` マクロ展開による新規 Clippy 警告が発生した場合は修正する
  - _Requirements: 5.3_

- [x] 5.3 英語設定時の動作を検証するテストを追加する
  - `language = "en"` を設定した場合に英語コメントが投稿されることを検証するテストケースを追加する（未知 locale での英語フォールバック動作も含む）
  - `TransitionUseCase` のモックテストで `en` 設定時の `comment_on_issue` 引数が英語文字列であることをアサートする
  - _Requirements: 4.1, 4.2, 4.3, 4.4_
