# Implementation Plan

- [x] 1. (P) steering/tech.md の CLIサブコマンド名を実装と一致させる
- [x] 1.1 (P) サブコマンド一覧の更新
  - `Subcommands: run / init / status` を `start / stop / init / status / doctor` に修正する
  - _Requirements: 1.1, 1.3_

- [x] 1.2 (P) 使用例コマンドの更新
  - `cargo run -- run` を `cargo run -- start` に修正する
  - _Requirements: 1.2, 1.3_

- [x] 2. (P) CHANGELOG.md に start/stop デーモン機能を記載する
  - v0.1.0 または `[Unreleased]` セクションの `### Added` に `cupola start --daemon` の説明を追記する
  - 同セクションに `cupola stop` の説明を追記する
  - 既存の CHANGELOG フォーマット（Keep a Changelog 形式）に準拠する
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 3. (P) README に Unix only 制約を明記する
- [x] 3.1 (P) README.md の更新
  - Requirements または Prerequisites セクションに「Unix (macOS / Linux) only」を追記する
  - `nix` クレート (`cfg(unix)`) に起因する制約である旨を添える
  - _Requirements: 3.1, 3.3, 3.4_

- [x] 3.2 (P) README.ja.md の更新
  - 同セクションに「Unix (macOS / Linux) のみ対応」を追記する（日本語）
  - `nix` クレート (`cfg(unix)`) に起因する制約である旨を添える
  - _Requirements: 3.2, 3.3, 3.4_

- [x] 4. (P) .gitignore に .env を追加してトークン漏洩リスクを排除する
  - `.gitignore` に `.env` エントリが存在しないことを確認してから追加する
  - `.env.example` 等のサンプルファイルが除外対象にならないよう `/env` ではなく `.env` を記述する
  - _Requirements: 4.1, 4.2, 4.3_
