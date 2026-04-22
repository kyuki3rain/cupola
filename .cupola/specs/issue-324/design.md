# Design Document: clippy::unwrap_used を deny に追加

## Overview

本機能は、`Cargo.toml` の `[lints.clippy]` セクションに `unwrap_used = "deny"` を追加し、`src/lib.rs` のテスト用 `allow` 属性を拡張することで、本番コードへの `.unwrap()` 混入を型システムレベルで防ぐ防御的措置を実施する。

実測により本番コードに `.unwrap()` が 0 件であることが確認済みのため、既存コードの修正は一切不要。設定ファイル 2 箇所への計 2 行追加のみで完結する。

### Goals

- `Cargo.toml` に `unwrap_used = "deny"` を追加し、本番コードへの `.unwrap()` 混入を clippy でブロックする
- テストコードでは `cfg_attr(test, allow(...))` により `.unwrap()` を引き続き許容する
- `devbox run clippy` および `devbox run test` がエラーなく通過する

### Non-Goals

- テストコード内の既存 `.unwrap()` を `.expect()` に書き換えること（scope 外）
- 本番コードの修正（実測で 0 件のため不要）

## Architecture

### Existing Architecture Analysis

本変更は Cargo の lint 設定とクレートレベルの属性マクロに閉じた変更であり、アプリケーションアーキテクチャ（Clean Architecture 4 層）には影響しない。

既存のパターン:
- `Cargo.toml[lints.clippy]`: `expect_used = "deny"` が設定済み
- `src/lib.rs`: `#![cfg_attr(test, allow(clippy::expect_used))]` が設定済み

本変更はこれらと完全対称のパターンを踏襲する。

### Architecture Pattern & Boundary Map

変更対象は設定ファイルのみ。アーキテクチャ境界に変更なし。

| 変更対象 | 変更内容 | 影響範囲 |
|---------|---------|---------|
| `Cargo.toml` | `unwrap_used = "deny"` を `[lints.clippy]` に追加 | プロジェクト全体の clippy lint 設定 |
| `src/lib.rs` | `cfg_attr` の allow リストに `clippy::unwrap_used` を追加 | テストコードのみ |

### Technology Stack

| 層 | ツール/バージョン | 本機能での役割 | 備考 |
|---|-----------------|--------------|------|
| 静的解析 | Rust Clippy (stable) | `unwrap_used` lint の適用 | `Cargo.toml` の `[lints.clippy]` で制御 |
| ビルドシステム | Cargo (Rust Edition 2024) | lint 設定の読み込みと適用 | 追加ライブラリ不要 |

## Requirements Traceability

| Requirement | Summary | 変更対象コンポーネント | Flows |
|-------------|---------|---------------------|-------|
| 1.1 | `Cargo.toml` に `unwrap_used = "deny"` を含む | `Cargo.toml` [lints.clippy] | — |
| 1.2 | `devbox run clippy` がゼロ件エラー | `Cargo.toml` [lints.clippy] | clippy 実行 |
| 1.3 | 本番コードへの `.unwrap()` 追加で clippy エラー | `Cargo.toml` [lints.clippy] | clippy 実行 |
| 1.4 | `expect_used = "deny"` を変更せず保持 | `Cargo.toml` [lints.clippy] | — |
| 2.1 | `src/lib.rs` に `allow(clippy::unwrap_used)` を追加 | `src/lib.rs` クレート属性 | — |
| 2.2 | `devbox run test` が全テスト通過 | `src/lib.rs` クレート属性 | テスト実行 |
| 2.3 | テストコードで `.unwrap()` を許容 | `src/lib.rs` クレート属性 | テスト実行 |
| 2.4 | `clippy::expect_used` の allow 設定を保持 | `src/lib.rs` クレート属性 | — |

## Components and Interfaces

| コンポーネント | 層 | Intent | Req Coverage | 契約 |
|--------------|---|--------|--------------|------|
| `Cargo.toml` [lints.clippy] | ビルド設定 | clippy lint ルールの定義 | 1.1, 1.2, 1.3, 1.4 | State |
| `src/lib.rs` クレート属性 | ビルド設定 | テストスコープでの lint 除外 | 2.1, 2.2, 2.3, 2.4 | State |

### ビルド設定

#### `Cargo.toml` [lints.clippy] セクション

| Field | Detail |
|-------|--------|
| Intent | プロジェクト全体の Clippy lint ルールを一元管理する |
| Requirements | 1.1, 1.2, 1.3, 1.4 |

**Responsibilities & Constraints**

- `unwrap_used = "deny"` を `expect_used = "deny"` の直下に追加する
- `all = { level = "warn", priority = -1 }` の基底設定は変更しない
- 追加後のエントリ順序: `all` → `expect_used` → `unwrap_used`

**Contracts**: State [x]

##### State Management

- 変更前: `unwrap_used` は `all = "warn"` により warn レベル
- 変更後: `unwrap_used = "deny"` により deny レベルに昇格
- 本番コード 0 件確認済みのため、既存ビルドへの破壊的影響なし

**Implementation Notes**

- Integration: 既存の `expect_used = "deny"` エントリの直後に追記する
- Validation: `devbox run clippy` で確認
- Risks: なし（本番コード 0 件確認済み）

---

#### `src/lib.rs` クレートレベル属性

| Field | Detail |
|-------|--------|
| Intent | テストスコープでの `unwrap_used` lint を除外し、テストコードの `.unwrap()` 使用を許容する |
| Requirements | 2.1, 2.2, 2.3, 2.4 |

**Responsibilities & Constraints**

- 既存の `#![cfg_attr(test, allow(clippy::expect_used))]` を `#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]` に拡張する
- ファイル先頭の属性行のみを変更する。他の行は変更しない

**Contracts**: State [x]

##### State Management

- 変更前: テストで `expect_used` のみ許容
- 変更後: テストで `expect_used` および `unwrap_used` を許容
- `cfg(test)` スコープのみに適用されるため、本番コードへの影響はない

**Implementation Notes**

- Integration: `#![cfg_attr(test, allow(...))]` の allow リストにカンマ区切りで `clippy::unwrap_used` を追加するだけ
- Validation: `devbox run test` で確認
- Risks: なし

## Testing Strategy

### Unit Tests

- 本変更は設定ファイルへの追加のみのため、専用ユニットテストは不要
- `devbox run clippy` による静的解析が受け入れテストの役割を果たす

### Integration Tests

- `devbox run test`: 既存テストスイート全体が `unwrap_used` の影響なく通過することを確認する
- `devbox run clippy`: `--lib` ターゲットでエラー 0 件を確認する
