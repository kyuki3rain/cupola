# Research & Design Decisions

---
**Purpose**: `clippy::unwrap_used = "deny"` 追加に関する調査結果と設計判断を記録する。

---

## Summary

- **Feature**: `issue-324` — clippy::unwrap_used を deny に追加 (本番コード 0 件・防御的措置)
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - 本番コード (`--lib`) に `.unwrap()` は実測で 0 件。即時適用にコスト不要
  - テストコード (`--all-targets`) には 257 件存在するが、既存の `expect_used` と同じパターンで `cfg_attr(test, allow(...))` により除外可能
  - `Cargo.toml` と `src/lib.rs` の 2 箇所のみ変更。他ファイルへの影響なし

## Research Log

### 既存 lint 設定の調査

- **Context**: `unwrap_used` を追加する前に、既存の `expect_used` がどのように設定・除外されているかを確認した。
- **Sources Consulted**: `Cargo.toml` (行 41-43)、`src/lib.rs` (行 1)、ステアリングファイル `tech.md`
- **Findings**:
  - `Cargo.toml` の `[lints.clippy]` に `all = { level = "warn", priority = -1 }` および `expect_used = "deny"` が設定済み
  - `src/lib.rs` 先頭に `#![cfg_attr(test, allow(clippy::expect_used))]` が存在し、テストでの `expect_used` を許容している
  - `unwrap_used` は未設定 (= warn に留まっている状態)
- **Implications**: `unwrap_used` の追加は `expect_used` と完全に対称的なパターンで実施できる。2 行追加のみ。

### unwrap_used 違反数の実測

- **Context**: 追加前に本番コードへの影響規模を確認した。
- **Sources Consulted**: `devbox run -- cargo clippy -- -D clippy::unwrap_used` の実行結果 (Issue #324 本文より)
- **Findings**:
  - `--all-targets` (テスト含む): 257 件
  - `--lib` (本番コードのみ): **0 件**
- **Implications**: 本番コードの修正は一切不要。`cfg_attr(test, allow(...))` を追加するだけでテスト CI も通過する。

## Design Decisions

### Decision: cfg_attr による除外戦略

- **Context**: テストコードでは `.unwrap()` が慣用的に使われており、257 件すべてを `.expect()` に書き換えることは scope 外。
- **Alternatives Considered**:
  1. すべての `.unwrap()` を `.expect()` に書き換える — scope が大幅に広がり不要なリスクを伴う
  2. `cfg_attr(test, allow(clippy::unwrap_used))` を追加する — 既存の `expect_used` と同一パターンで整合性が高い
- **Selected Approach**: オプション 2。`src/lib.rs` の `cfg_attr` 属性を拡張して `clippy::unwrap_used` を追加する。
- **Rationale**: `expect_used` と完全対称であり、Rust 慣習（テストでは unwrap/expect 許容）とも整合する。変更コストが最小。
- **Trade-offs**: テストコードでの `.unwrap()` 使用は継続して許容されるため、テスト品質向上の観点では `.expect()` への移行が望ましい場面もあるが、今回の scope 外。
- **Follow-up**: 将来的に新規テストでは `.expect("reason")` を推奨する慣行を維持する（既存 `rust-testing.md` のガイドラインで対応済み）。

## Risks & Mitigations

- 本番コードに `.unwrap()` が追加された場合、clippy エラーで即座に検出される — これは意図した挙動であり、リスクではなく目的
- `cfg_attr` の対象が `#[cfg(test)]` モジュールのみのため、integration tests (`tests/` ディレクトリ) が適切に除外されるか — `--lib` フラグと `cfg(test)` の組み合わせは Cargo の標準動作で保証されており問題なし

## References

- [Rust Clippy — unwrap_used](https://rust-lang.github.io/rust-clippy/master/index.html#unwrap_used) — lint の詳細説明
- `tech.md` — プロジェクト技術スタック・コード品質基準
