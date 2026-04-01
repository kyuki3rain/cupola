# 実装タスク一覧

## v0-1-0-release-prep

---

- [x] 1. CHANGELOG.md の作成
- [x] 1.1 (P) Keep a Changelog 形式の CHANGELOG.md をリポジトリルートに作成する
  - `# Changelog` ヘッダーをファイル先頭に記載する
  - `## [Unreleased]` セクションを先頭に追加する（将来の変更記録用）
  - `## [0.1.0] - 2026-04-01` セクションを追加する
  - `### Added` サブセクションに以下の8機能を箇条書きで記載する：GitHub Issue 検知と cc-sdd による設計自動生成、設計/実装 PR の自動作成、Review thread への自動修正・返信・resolve、CI 失敗・conflict の自動検知と修正、同時実行数制限（max_concurrent_sessions）、モデル指定（cupola.toml + Issue ラベル）、`cupola doctor` / `cupola init` コマンド、Graceful shutdown
  - ファイル末尾に `[0.1.0]: https://github.com/kyuki3rain/cupola/releases/tag/v0.1.0` のタグリンクを追加する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [x] 2. Cargo.toml パッケージメタデータの追加
- [x] 2.1 (P) Cargo.toml の `[package]` セクションにリリースメタデータフィールドを追加する
  - `description = "A locally-resident agent that automates GitHub Issue-driven development using Claude Code"` を追加する
  - `repository = "https://github.com/kyuki3rain/cupola"` を追加する
  - `readme = "README.md"` を追加する
  - `keywords = ["github", "automation", "agent", "claude", "ci"]` を追加する
  - `categories = ["command-line-utilities", "development-tools"]` を追加する
  - `version = "0.1.0"` が変更されていないことを確認する
  - `[dependencies]` 等の他セクションへの変更がないことを確認する
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_

- [x] 3. ビルド・品質チェックの実行と確認
- [x] 3.1 cargo fmt を実行してコードフォーマットの差分がないことを確認する
  - `cargo fmt` を実行してフォーマットを適用する
  - `cargo fmt -- --check` がゼロ差分で通ることを確認する
  - _Requirements: 3.4_

- [x] 3.2 cargo clippy を実行して警告がゼロであることを確認する
  - `RUSTFLAGS="-D warnings" cargo clippy --all-targets` を実行する（CI と同一の設定）
  - 警告・エラーがゼロであることを確認する
  - _Requirements: 3.3_

- [x] 3.3 cargo build と cargo test を実行してリグレッションがないことを確認する
  - `cargo build` がエラーなく完了することを確認する
  - `cargo test --lib -- --test-threads=1` が全テストパスすることを確認する
  - _Requirements: 3.1, 3.2_
