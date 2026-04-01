# Research & Design Decisions

---
**Purpose**: i18n-github-comments フィーチャーの設計調査記録

---

## Summary

- **Feature**: `i18n-github-comments`
- **Discovery Scope**: Extension（既存システムへの拡張）
- **Key Findings**:
  - `rust-i18n` v3 は per-call locale 指定（`locale = &lang`）をサポートし、グローバル state の変更なしに翻訳可能
  - `t!()` マクロは `String` を返すため、`&str` を期待する `unwrap_or` との組み合わせは変数への事前バインドが必要
  - 既存の統合テストが日本語文字列を直接 `contains` で検証しているため、テスト更新が必要

---

## Research Log

### rust-i18n v3 API と per-call locale

- **Context**: グローバル locale 設定を変更せず per-call で locale を切り替えたい
- **Sources Consulted**: rust-i18n crate documentation (crates.io), issue tracker
- **Findings**:
  - `t!("key", locale = "en")` の形式で per-call locale 指定が可能
  - `t!()` マクロの戻り値型は `String`（v3 以降）
  - パラメータ補間は `t!("key", locale = "en", param = value)` の形式で行う
  - `rust_i18n::i18n!("locales", fallback = "en")` をクレートルート（`lib.rs`）に一度だけ配置する
  - YAML ファイルは `locales/<lang>.yml` のフラットまたはネスト構造をサポート
- **Implications**: グローバル state を汚染しない設計が可能。`Config.language` を毎回引数として渡すだけで多言語対応が完結する

### `unwrap_or` と `String` の型不整合

- **Context**: `issue.error_message.as_deref().unwrap_or("不明")` を i18n 化する際の型問題
- **Findings**:
  - `as_deref()` は `Option<String>` → `Option<&str>` を返す
  - `unwrap_or` に `&str` を期待するが、`t!()` は `String` を返す
  - 解決策: `let unknown = t!(...); issue.error_message.as_deref().unwrap_or(&unknown)` のパターンで変数に先にバインド
- **Implications**: フォールバック文字列の i18n 化は1行では書けないため、設計ではこのパターンを明示的に規定する必要がある

### 既存テストの影響範囲

- **Context**: `tests/integration_test.rs` L370 で `msg.contains("リトライ上限")` を使用
- **Findings**:
  - `language` がデフォルト `"ja"` の場合、`ja.yml` に `"リトライ上限"` を含む文字列を定義すれば既存テストはそのまま通過可能
  - ただし `en` 設定のテストケースを追加する場合は英語文字列を期待値にする必要がある
- **Implications**: 既存テストのアサーション文字列変更は不要（ja.yml の内容が現在のハードコード文字列と一致していれば）

### クレートルートへの macro 配置

- **Context**: `src/lib.rs` が現在4行のモジュール宣言のみ
- **Findings**:
  - `rust_i18n::i18n!("locales", fallback = "en")` をクレートルートに1回配置するだけでよい
  - `locales/` ディレクトリはクレートルート（`Cargo.toml` と同階層）に配置する
- **Implications**: `src/lib.rs` へのマクロ追加のみで `t!()` が全モジュールから使用可能になる

---

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| rust-i18n per-call locale | `t!(key, locale = lang)` を毎回の呼び出しで指定 | グローバル state 不変、並行安全、Cupola のアーキテクチャに自然 | マクロ呼び出しごとに locale 引数が必要 | 採用 |
| グローバル locale 設定 | `rust_i18n::set_locale(lang)` で一括設定 | 呼び出しが簡潔 | スレッド安全でない、並行処理での混在リスク | 不採用：マルチセッション環境に不適合 |
| 独自翻訳マップ | `HashMap<(key, lang), String>` で管理 | 依存追加不要 | メンテナンスコスト大、標準的でない | 不採用：コスト対効果が低い |

---

## Design Decisions

### Decision: per-call locale vs グローバル locale

- **Context**: `Config.language` は Issue ごとのセッションで共有されるが、将来マルチテナント化される可能性がある
- **Alternatives Considered**:
  1. グローバル locale — `rust_i18n::set_locale()` で起動時に設定
  2. per-call locale — `t!()` 呼び出しごとに `locale = &self.config.language` を渡す
- **Selected Approach**: per-call locale
- **Rationale**: tokio の非同期並行処理環境でグローバル state を変更すると、並行セッションが互いに干渉するリスクがある。また、Cupola のクリーンアーキテクチャ原則（副作用の局所化）に合致する
- **Trade-offs**: 各 `t!()` 呼び出しに引数が1つ増えるが、型安全性と並行安全性を確保できる
- **Follow-up**: マルチセッション環境での動作確認

### Decision: locales/ ディレクトリの配置場所

- **Context**: Rust プロジェクトの規約と rust-i18n の推奨場所
- **Selected Approach**: `locales/` をクレートルート（`Cargo.toml` と同階層）に配置
- **Rationale**: `rust_i18n::i18n!("locales", ...)` のデフォルトパス解決がクレートルート基準

---

## Risks & Mitigations

- `t!()` の戻り値型（`String`）と `unwrap_or` の期待型（`&str`）の不整合 — 変数への事前バインドパターンを設計で明示
- 未知 locale 指定時の fallback 動作の確認漏れ — テスト戦略に「未知 locale → en fallback」のテストケースを追加
- `cargo clippy` での `t!()` マクロ展開による新規 warning 発生リスク — 実装後に clippy を実行して確認

---

## References

- rust-i18n crate (crates.io) — per-call locale API と YAML フォーマット仕様
