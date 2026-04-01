# 要件定義書

## プロジェクト説明（入力）
v0.1.0 リリース準備 - CHANGELOG.md の作成（Keep a Changelog 形式）と Cargo.toml へのパッケージメタデータ（description, repository, readme, keywords, categories）追加

## はじめに
cupola は GitHub Issues と PR をインターフェースとして利用し、設計から実装までを自動化するローカル常駐エージェントである。
v0.1.0 として OSS の正式リリースを行うにあたり、変更履歴の透明性を確保するための CHANGELOG.md の整備と、
Cargo.toml へのパッケージメタデータ追加によって crates.io 公開・利用者への情報提供を可能にする。

## 要件

### 要件 1: CHANGELOG.md の作成

**目的:** OSS メンテナーとして、Keep a Changelog 形式に準拠した CHANGELOG.md をリポジトリルートに作成し、v0.1.0 の主要な変更内容を記録することで、利用者が変更履歴を把握できるようにする。

#### 受け入れ基準
1. The cupola repository shall have a `CHANGELOG.md` file at the repository root.
2. When a user reads `CHANGELOG.md`, the cupola repository shall present the file in Keep a Changelog format (https://keepachangelog.com/) with a header, `## [Unreleased]` section, and a `## [0.1.0]` section.
3. The cupola repository shall list the following features under `## [0.1.0] - Added`: GitHub Issue 検知と cc-sdd による設計自動生成、設計/実装 PR の自動作成、Review thread への自動修正・返信・resolve、CI 失敗・conflict の自動検知と修正、同時実行数制限（max_concurrent_sessions）、モデル指定（cupola.toml + Issue ラベル）、cupola doctor / init コマンド、Graceful shutdown。
4. The cupola repository shall include a version date entry in the `## [0.1.0]` section header in the form `## [0.1.0] - YYYY-MM-DD`.
5. The cupola repository shall include a footer with comparison links (e.g., `[0.1.0]: https://github.com/.../compare/...`).

---

### 要件 2: Cargo.toml パッケージメタデータの追加

**目的:** OSS メンテナーとして、Cargo.toml の `[package]` セクションに必要なメタデータを追加することで、crates.io での公開準備を整え、利用者がパッケージの目的・出所を明確に把握できるようにする。

#### 受け入れ基準
1. The cupola `Cargo.toml` shall include a `description` field under `[package]` that succinctly describes the crate's purpose in English (under 200 characters).
2. The cupola `Cargo.toml` shall include a `repository` field under `[package]` containing the GitHub repository URL.
3. The cupola `Cargo.toml` shall include a `readme` field under `[package]` pointing to `README.md`.
4. The cupola `Cargo.toml` shall include a `keywords` field under `[package]` with up to 5 lowercase keywords relevant to the crate (e.g., `["github", "automation", "agent", "claude", "ci"]`).
5. The cupola `Cargo.toml` shall include a `categories` field under `[package]` with at least one valid crates.io category slug (e.g., `["command-line-utilities", "development-tools"]`).
6. The cupola `Cargo.toml` shall retain `version = "0.1.0"` without change.

---

### 要件 3: ビルドおよびテストの継続的合格

**目的:** 開発者として、メタデータの追加後もビルドとテストがすべて通過し、既存機能に影響を与えないことを確認する。

#### 受け入れ基準
1. When `cargo build` is executed after the changes, the cupola build system shall complete without errors or warnings.
2. When `cargo test` is executed after the changes, the cupola test suite shall pass all existing tests without regressions.
3. When `cargo clippy -- -D warnings` is executed after the changes, the cupola linter shall report zero warnings or errors.
4. When `cargo fmt --check` is executed after the changes, the cupola formatter shall report no formatting differences.
