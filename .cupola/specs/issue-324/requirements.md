# Requirements Document

## Project Description (Input)
## 概要

防御的措置として `Cargo.toml` の `[lints.clippy]` に `unwrap_used = "deny"` を追加する。本番コードに既に `.unwrap()` が存在しないことを確認済みのため、実質ゼロコストで将来の事故を型システムレベルで防ぐ。

## 現状調査 (実測)

`devbox run -- cargo clippy -- -D clippy::unwrap_used` を実行した結果:

| 対象 | unwrap_used 違反 |
|------|------------------|
| `--all-targets` (テスト含む) | 257 件 |
| `--lib` (本番コードのみ) | **0 件** |

257 件はすべてテストコード / `tests/` / `#[cfg(test)]` 内。本番コードに `.unwrap()` は存在しない。

### 既存設定

- `Cargo.toml:43`: `expect_used = "deny"` ✓ 既に機能
- `src/lib.rs:1`: `#![cfg_attr(test, allow(clippy::expect_used))]` ✓ テストで allow
- `unwrap_used`: 未設定

実測では `.unwrap()` は本番コードで 0 件、違反 257 件はすべてテストコードに限られるため、本番コードでは `unwrap_used = "deny"` を有効にしつつ、テストでは既存の `expect_used` と同様に allow する方針は現状の運用と整合している（`tech.md` の「Code Quality」セクション参照）。

## 対応

### 1. Cargo.toml に追加

```toml
[lints.clippy]
all = { level = "warn", priority = -1 }
expect_used = "deny"
unwrap_used = "deny"  # ← 追加
```

### 2. lib.rs でテストを allow

既存の `expect_used` と同じ扱い:

```rust
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
```

### 3. 本番コードの修正

**不要** (実測で 0 件)。

## 受け入れ条件

- `Cargo.toml` の `[lints.clippy]` に `unwrap_used = "deny"` が追加される
- `src/lib.rs` の `cfg_attr(test, allow(...))` に `clippy::unwrap_used` が追加される
- `devbox run clippy` が通過する
- `devbox run test` が通過する

## 注記

サブエージェントが当初報告した「1143 件残存、CI 機能していない可能性」は **完全な誤読**。実調査で本番コード 0 件を確認。

本 issue は実コード変更ゼロ (設定 2 行追加のみ) の防御的措置。将来誰かが本番コードに `.unwrap()` を追加しようとした際、clippy で即座にブロックされる保険となる。

## 関連

- **#316** (PostCiFixLimitComment): 本 issue 完了後、新しいコードが `.unwrap()` を含められないため、実装時の型安全性が自然に保たれる


## はじめに

本ドキュメントは、`clippy::unwrap_used` を Cargo.toml の lint 設定で deny に追加し、テストコードでは引き続き許容するための要件を定義する。既存の `expect_used = "deny"` と対称的な設定を追加することで、本番コードへの `.unwrap()` 混入を型システムレベルで防ぐ防御的措置である。

## Requirements

### Requirement 1: Cargo.toml へのlint設定追加

**Objective:** 開発者として、`Cargo.toml` の `[lints.clippy]` セクションに `unwrap_used = "deny"` を追加したい。これにより、本番コードへの `.unwrap()` 混入を clippy がビルド時に即座に検出・拒否できるようにする。

#### Acceptance Criteria

1. The `Cargo.toml` shall `[lints.clippy]` セクションに `unwrap_used = "deny"` エントリを含む。
2. When `devbox run clippy` を実行した場合, the Clippy shall 本番コード (`--lib`) に対してゼロ件のエラーを報告する。
3. If 本番コードに `.unwrap()` が追加された場合, the Clippy shall `clippy::unwrap_used` エラーを報告しビルドを失敗させる。
4. The `Cargo.toml` shall 既存の `expect_used = "deny"` エントリを変更せず保持する。

### Requirement 2: ライブラリ内テストコードでの unwrap_used 許容

**Objective:** 開発者として、`src/lib.rs` の `cfg_attr(test, allow(...))` 属性に `clippy::unwrap_used` を追加したい。これにより、ライブラリクレート内の `#[cfg(test)]` テストコードでは従来どおり `.unwrap()` を使用できる状態を維持する。

なお、`src/lib.rs` の `cfg_attr(test, allow(...))` が効くのはライブラリクレート自身の unit test のみであり、`tests/` 配下の integration test には適用されない。`devbox run clippy`（`--all-targets` 含む）を通過するためには、integration test クレート側にも個別に `#![allow(clippy::unwrap_used)]` を付与するか、integration test では `.unwrap()` を使用しない制約を設けることが別途必要である。

#### Acceptance Criteria

1. The `src/lib.rs` shall `#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]` を先頭に含む。
2. When `devbox run test` を実行した場合, the テストスイート shall `clippy::unwrap_used` に起因する失敗なく全テストが通過する。
3. While `src/lib.rs` 内の `#[cfg(test)]` ブロックが実行される場合, the Clippy shall `.unwrap()` 使用を許容する。
4. The `src/lib.rs` shall 既存の `clippy::expect_used` の allow 設定を変更せず保持し、この allow はライブラリクレート内テストに対して適用される。
