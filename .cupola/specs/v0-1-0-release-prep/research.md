# Research & Design Decisions

---
**Purpose**: v0-1-0-release-prep の設計調査・意思決定ログ

---

## Summary
- **Feature**: `v0-1-0-release-prep`
- **Discovery Scope**: Simple Addition（既存プロジェクトへのドキュメント・メタデータ追加）
- **Key Findings**:
  - Keep a Changelog 形式は `## [Unreleased]` → `## [バージョン] - 日付` → 末尾のリンク定義という3段構成が標準
  - `cargo publish` に必要な Cargo.toml フィールドは `description`, `repository`, `readme`, `license` であり、`keywords`（最大5個）と `categories`（crates.io スラッグ）は任意だが OSS 公開時の慣例として推奨される
  - プロジェクトの既存コードへの変更は不要（ドキュメントとメタデータのみ）

## Research Log

### Keep a Changelog フォーマット仕様
- **Context**: CHANGELOG.md を標準形式で作成するための仕様確認
- **Sources Consulted**: https://keepachangelog.com/ja/1.1.0/
- **Findings**:
  - ファイル先頭に `# Changelog` ヘッダー
  - `## [Unreleased]` セクションを常に維持
  - `## [x.y.z] - YYYY-MM-DD` 形式でバージョンセクション
  - 変更種別: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`
  - ファイル末尾に比較リンク: `[x.y.z]: https://github.com/owner/repo/compare/vA.B.C...vX.Y.Z`
- **Implications**: v0.1.0 が最初のリリースのため、比較リンクは `vX.Y.Z...HEAD` 形式または初回タグとして記述

### Cargo.toml パッケージメタデータ仕様
- **Context**: crates.io 公開および利用者への情報提供のための必要フィールド確認
- **Sources Consulted**: https://doc.rust-lang.org/cargo/reference/manifest.html
- **Findings**:
  - `description`: 必須（crates.io 表示用）。1行、200文字以内推奨
  - `repository`: GitHub URL（`https://github.com/owner/repo` 形式）
  - `readme`: `README.md` へのパス
  - `keywords`: 最大5個、小文字英数字とハイフンのみ
  - `categories`: crates.io の有効なスラッグ（`command-line-utilities`, `development-tools` など）
  - `license`: 既存の `Apache-2.0` で問題なし
- **Implications**: cupola は CLI ツールかつ開発自動化ツールなので `command-line-utilities` と `development-tools` が適切なカテゴリ

### 既存コードへの影響分析
- **Context**: メタデータ変更が既存のビルド・テストに影響しないか確認
- **Findings**:
  - Cargo.toml のパッケージメタデータフィールドはコンパイルに影響しない
  - CHANGELOG.md は新規作成のみ（既存ファイルなし）
  - `cargo build`, `cargo test`, `cargo clippy` への影響なし
- **Implications**: 品質チェック（cargo fmt / clippy / test）は既存のものをそのまま通過するはず

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| ドキュメント直接作成 | CHANGELOG.md をリポジトリルートに直接作成 | シンプル、標準的 | なし | 採用 |
| Cargo.toml 直接編集 | 既存 Cargo.toml の `[package]` セクションに追記 | 既存構造を維持 | なし | 採用 |

## Design Decisions

### Decision: v0.1.0 リリース日の記載
- **Context**: CHANGELOG.md の `## [0.1.0]` セクションに日付を記載する必要がある
- **Alternatives Considered**:
  1. Issue 作成日（不明）を使用
  2. 2026-04-01（現在日）を使用
- **Selected Approach**: `2026-04-01` を使用（現在日）
- **Rationale**: 設計書作成日 = リリース準備完了予定日として適切
- **Trade-offs**: 実際のリリース日と異なる可能性があるが、実装時に修正可能

### Decision: keywords の選定
- **Context**: Cargo.toml の `keywords` に最大5個のキーワードを設定
- **Selected Approach**: `["github", "automation", "agent", "claude", "ci"]`
- **Rationale**: cupola の主要機能（GitHub 連携、自動化、AI エージェント）を表すキーワード

### Decision: categories の選定
- **Context**: crates.io の有効なカテゴリスラッグを選定
- **Selected Approach**: `["command-line-utilities", "development-tools"]`
- **Rationale**: cupola は CLI ツール（`cupola run/init/status`）であり、開発プロセス自動化ツールでもある

## Risks & Mitigations
- リリース日が実際のタグ打ち日と異なる可能性 — 実装フェーズで最終確認・修正する
- crates.io カテゴリスラッグの有効性 — `command-line-utilities`, `development-tools` は標準的なスラッグで問題なし

## References
- [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) — CHANGELOG 形式の公式仕様
- [Cargo.toml manifest reference](https://doc.rust-lang.org/cargo/reference/manifest.html) — Cargo パッケージメタデータの公式ドキュメント
- [crates.io categories](https://crates.io/categories) — 有効なカテゴリスラッグ一覧
