# 実装タスク: oss-community-files

## タスク一覧

- [ ] 1. (P) CONTRIBUTING.md の作成
  - リポジトリルートに英語で記述した `CONTRIBUTING.md` を新規作成する
  - Welcome / Project Overview セクション: cupola の概要と外部コントリビュータ向けの歓迎メッセージを記載する
  - Prerequisites セクション: Rust stable、devbox、gh CLI を前提ツールとして明記する
  - Development Environment Setup セクション: `devbox shell` を最初のステップとして、続けて `cargo build`・`cargo test` の手順を記載する
  - How to Contribute セクション: バグ報告（Issue 作成）・機能提案（Issue + ラベル）・PR 手順をそれぞれ説明する
  - PR Process セクション: ブランチ命名規則（`feat/`・`fix/`・`docs/` 等）、conventional commits 形式（`feat:`・`fix:`・`docs:` 等）、GitHub Actions CI が全通過することを PR マージ条件として記載する
  - Coding Standards セクション: `cargo fmt`・`cargo clippy --all-targets` をコーディング規約として明記し、テスト追加の期待を記述する
  - 使用技術スタック（Rust Edition 2024、devbox、cargo、GitHub Actions）をドキュメント内容に正確に反映する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 6.1, 6.2_

- [ ] 2. (P) SECURITY.md の作成
  - リポジトリルートに英語で記述した `SECURITY.md` を新規作成する
  - Supported Versions セクション: 現時点では最新リリースのみをサポート対象として記載する
  - Reporting a Vulnerability セクション: GitHub Private Vulnerability Reporting（`https://github.com/kyuki3rain/cupola/security/advisories/new`）を主要な報告手段として明示する
  - 公開 Issue での脆弱性報告を禁止する旨を明確に記述する
  - Response Timeline セクション: 「確認: 48 時間以内」「修正: 90 日以内」の対応タイムラインを明示する
  - 報告者クレジットポリシー（開示の同意を得た場合にクレジットを付与する）を含める
  - cupola の現在の公開状況（最新リリースのみサポート）をサポートバージョンセクションに正確に反映する
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 6.3_

- [ ] 3. (P) CODE_OF_CONDUCT.md の作成
  - リポジトリルートに `CODE_OF_CONDUCT.md` を新規作成する
  - Contributor Covenant v2.1 の公式英語テキストをそのまま使用する
  - 連絡先プレースホルダー `[INSERT CONTACT METHOD]` を GitHub Issues URL（`https://github.com/kyuki3rain/cupola/issues`）に置換する
  - その他のプレースホルダーがないことを確認し、公式テキストからの逸脱がないようにする
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [ ] 4. README バッジの追加
- [ ] 4.1 (P) README.md にバッジを追加
  - `README.md` のタイトル行（`# Cupola`）の直後、既存の `[日本語](./README.ja.md)` 行の前にバッジ行を挿入する
  - 追加する CI バッジ: `[![CI](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml/badge.svg)](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml)`
  - 追加する License バッジ: `[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/kyuki3rain/cupola/blob/main/LICENSE)`
  - crates.io / docs.rs バッジは追加しない（crates.io 公開前のため）
  - _Requirements: 4.1, 4.2, 4.4_

- [ ] 4.2 (P) README.ja.md にバッジを追加
  - `README.ja.md` のタイトル行直後に、README.md と同等の CI バッジおよび License バッジを挿入する
  - バッジの内容・URL は README.md と完全に同一にする
  - crates.io / docs.rs バッジは追加しない（crates.io 公開前のため）
  - _Requirements: 4.3, 4.4_

- [ ] 5. (P) .github/dependabot.yml の作成
  - `.github/dependabot.yml` を新規作成する（既存の `.github/` ディレクトリに追加）
  - `version: 2` を指定し、GitHub Dependabot 設定仕様に準拠する
  - `cargo` エコシステム: ルートディレクトリ（`/`）対象、`weekly`（`monday`）スケジュール、commit prefix `chore(deps)`、ラベル `dependencies` を設定する
  - `github-actions` エコシステム: `weekly`（`monday`）スケジュール、commit prefix `ci(deps)`、ラベル `ci` + `dependencies` を設定する
  - Cargo の `minor` および `patch` 更新を `rust-dependencies` グループとしてバンドルし、PR を1件にまとめる
  - GitHub Actions の全更新を `github-actions` グループとしてバンドルし、PR を1件にまとめる
  - YAML 構文が正しいことを確認する
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 6.4_
