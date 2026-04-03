# Research & Design Decisions

---
**Purpose**: discover フェーズの調査結果とアーキテクチャ判断の根拠を記録する。

---

## Summary

- **Feature**: `replace-expect-with-context`
- **Discovery Scope**: Extension（既存applicationレイヤーのエラーハンドリング変更）
- **Key Findings**:
  - `build_session_config` は `src/application/prompt.rs:22` で `SessionConfig` を返す純粋関数。戻り値型を `Result<SessionConfig, anyhow::Error>` に変更することで、呼び出し元へのエラー伝播が可能になる
  - `step7_spawn_processes`（`src/application/polling_use_case.rs`）がすでに各Issueをループ処理しており、`build_session_config` のエラーを `continue` でスキップするパターンが自然に適合する
  - `Cargo.toml` の `[lints.clippy]` では `all` が `all = { level = "warn", priority = -1 }` に更新されているため、`expect_used = "deny"` は単純な追記ではなく、実際の設定形式に合わせて記述を更新する必要がある

## Research Log

### expect() 箇所の特定

- **Context**: Issue本文に記載された3箇所の確認
- **Sources Consulted**: `src/application/prompt.rs`, `src/application/transition_use_case.rs`
- **Findings**:
  - `prompt.rs:36` — `pr_number.expect("fixing state requires pr_number in DB")` （DesignFixing）
  - `prompt.rs:47` — 同パターン（ImplementationFixing）
  - `transition_use_case.rs:66` — `.expect("just reset")`（reset直後のfind_by_id）
- **Implications**: 3箇所とも application レイヤー内。ドメイン層には影響なし

### build_session_config の呼び出し元分析

- **Context**: 戻り値型変更の波及範囲確認
- **Sources Consulted**: `src/application/polling_use_case.rs:699-706`
- **Findings**:
  - `step7_spawn_processes` 内のループ処理から呼ばれる
  - 既存コードはエラー時に `continue` でスキップするパターンを複数箇所で使用済み
- **Implications**: `if let Err(e) = build_session_config(...) { warn!(...); continue; }` パターンが既存コードと一致し、差分が最小化される

### transition_use_case の呼び出し元分析

- **Context**: `step6_apply_events` でのエラーハンドリング確認
- **Sources Consulted**: `src/application/polling_use_case.rs:526-566`
- **Findings**:
  - `step6_apply_events` はすでに `if let Err(e)` でエラーをハンドルしている
  - `transition_use_case.rs:66` の `?` 伝播は自然に上位でキャッチされる
- **Implications**: 呼び出し元の変更は不要。`?` 演算子追加のみ

### Cargo.toml の lint 設定確認

- **Context**: `expect_used = "deny"` 追加の要件
- **Sources Consulted**: `Cargo.toml:36-37`
- **Findings**:
  - `[lints.clippy]` セクションは既存。`all = { level = "warn", priority = -1 }` に更新済み
  - `expect_used = "deny"` を同セクションへ追記することで対応可能
- **Implications**: セクション新規作成は不要

### src/lib.rs の現状確認

- **Context**: `cfg_attr` でテストコードの `expect_used` を許可する追記箇所
- **Sources Consulted**: `src/lib.rs`（43行）
- **Findings**:
  - ファイルは存在。i18n 初期化とモジュール宣言を含む
  - `#![cfg_attr(...)]` アトリビュートを先頭付近に追記する形で対応可能
- **Implications**: ファイル書き換えが必要。既存コードへの影響なし

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| `anyhow::Error` を使用 | `Result<SessionConfig, anyhow::Error>` | applicationレイヤーの既存エラー型と統一 | - | ステアリングで application は thiserror だが、anyhow::Context が既に利用されている |
| カスタムエラー型 | `SessionConfigError` を新設 | 型安全性が高い | 変更範囲が増大し、費用対効果が低い | この変更スコープには過剰 |

→ `anyhow::Error`（`anyhow::Context` trait の `?` 演算子）を採用

## Design Decisions

### Decision: build_session_config の戻り値型

- **Context**: panicを排除しつつ呼び出し元への最小変更で対応する
- **Alternatives Considered**:
  1. `Result<SessionConfig, anyhow::Error>` — 既存の anyhow パターンと統一
  2. `Result<SessionConfig, SessionConfigError>` — 専用エラー型の新設
- **Selected Approach**: `Result<SessionConfig, anyhow::Error>`
- **Rationale**: applicationレイヤーですでに `anyhow::Context` を利用しており、コードの一貫性を保ちつつ変更範囲を最小化できる
- **Trade-offs**: thiserror の厳格な型安全性は得られないが、このユースケースでは問題なし
- **Follow-up**: 将来的に `SessionConfigError` に移行する場合は独立したリファクタリングとして実施

## Risks & Mitigations

- `lib.rs` への `cfg_attr` 追記後に既存テストが壊れる可能性 — `cargo test` で確認
- `expect_used = "deny"` 追加後に他の箇所で未検出の `expect()` が残存している可能性 — `cargo clippy` のエラーを全件解消してからコミット
- `build_session_config` の戻り値型変更が他の呼び出し元に波及する可能性 — grep で呼び出し箇所を全件確認済み（`polling_use_case.rs` 1箇所のみ）

## References

- anyhow クレート: context/with_context パターン — エラーにコンテキストメッセージを付加する標準的手法
- clippy lint `expect_used`: `clippy::expect_used` — `expect()` 呼び出しを静的に禁止する lint
