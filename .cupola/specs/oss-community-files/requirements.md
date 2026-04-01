# 要件定義書

## はじめに

cupola プロジェクトを OSS として外部コントリビュータが参加しやすい環境を整えるため、コミュニティ標準ファイル群を追加する。対象ファイルは CONTRIBUTING.md・SECURITY.md・CODE_OF_CONDUCT.md・README バッジ・.github/dependabot.yml の 5 種類であり、プロジェクトの実態（Rust / devbox / GitHub Actions / Clean Architecture）と整合した内容にする。

---

## 要件

### 要件 1: CONTRIBUTING.md の作成

**目的:** 外部コントリビュータとして、貢献方法・開発環境セットアップ・コーディング規約を一箇所で把握したい。そうすることで、プロジェクトへの参加障壁を下げ、円滑にコントリビューションできる。

#### 受け入れ基準

1. The CONTRIBUTING.md shall ファイルルートに配置し、英語で記述されていること。
2. The CONTRIBUTING.md shall 以下のセクションを含むこと: Welcome / Project Overview、Prerequisites（Rust stable, devbox, gh CLI）、Development Environment Setup（`devbox shell`, `cargo build`, `cargo test`）、How to Contribute（バグ報告、機能提案、PR）、PR Process（ブランチ命名、conventional commits、CI チェック）、Coding Standards（`cargo fmt`, `cargo clippy --all-targets`, テスト）。
3. When コントリビュータが開発環境を構築する場合、the CONTRIBUTING.md shall `devbox shell` コマンドを最初のセットアップステップとして明示すること。
4. The CONTRIBUTING.md shall ブランチ命名規則として `feat/`, `fix/`, `docs/` などの conventional prefix を例示すること。
5. The CONTRIBUTING.md shall conventional commits 形式（`feat:`, `fix:`, `docs:` 等）を commit メッセージの標準として明記すること。
6. The CONTRIBUTING.md shall cargo clippy `--all-targets` と `cargo fmt` をコーディング規約として明記すること。
7. The CONTRIBUTING.md shall CI チェック（GitHub Actions）が全て通ることを PR マージ条件として記述すること。

---

### 要件 2: SECURITY.md の作成

**目的:** セキュリティ研究者・ユーザーとして、脆弱性を安全かつ迅速に報告する方法を知りたい。そうすることで、プロジェクトのセキュリティリスクを適切に管理できる。

#### 受け入れ基準

1. The SECURITY.md shall ファイルルートに配置し、英語で記述されていること。
2. The SECURITY.md shall GitHub Private Vulnerability Reporting（`https://github.com/kyuki3rain/cupola/security/advisories/new`）を脆弱性報告の主要手段として明記すること。
3. The SECURITY.md shall 公開 Issue での脆弱性報告を禁止する旨を明記すること。
4. The SECURITY.md shall 対応タイムラインとして「確認: 48時間以内」「修正: 90日以内」を明示すること。
5. The SECURITY.md shall サポート対象バージョンとして現時点では最新リリースのみをサポートする旨を記述すること。
6. If 脆弱性が報告された場合、the SECURITY.md shall 報告者に対するクレジット（開示の同意を得た場合）の方針を含むこと。

---

### 要件 3: CODE_OF_CONDUCT.md の作成

**目的:** コミュニティメンバーとして、プロジェクトにおける行動規範を理解したい。そうすることで、健全で包括的なコミュニティを維持できる。

#### 受け入れ基準

1. The CODE_OF_CONDUCT.md shall ファイルルートに配置されていること。
2. The CODE_OF_CONDUCT.md shall Contributor Covenant v2.1 の公式テキストを使用すること。
3. The CODE_OF_CONDUCT.md shall 連絡先として GitHub の Issues または適切なメールアドレスを設定すること。
4. The CODE_OF_CONDUCT.md shall Contributor Covenant の標準的な英語テキストを含むこと。

---

### 要件 4: README バッジの追加

**目的:** リポジトリ訪問者として、プロジェクトの CI ステータスとライセンスを一目で確認したい。そうすることで、プロジェクトの品質とライセンス条件を素早く把握できる。

#### 受け入れ基準

1. The README.md shall ファイル先頭（タイトル直下）に CI バッジ（`[![CI](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml/badge.svg)](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml)`）を追加すること。
2. The README.md shall CI バッジと同じ位置に License バッジ（`[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/kyuki3rain/cupola/blob/main/LICENSE)`）を追加すること。
3. The README.ja.md shall README.md と同等のバッジ（CI・License）をファイル先頭に追加すること。
4. When crates.io への公開が完了していない場合、the README.md shall crates.io および docs.rs バッジを追加しないこと。

---

### 要件 5: .github/dependabot.yml の作成

**目的:** プロジェクトメンテナーとして、Cargo および GitHub Actions の依存関係を自動更新したい。そうすることで、セキュリティパッチや最新版の追跡を省力化できる。

#### 受け入れ基準

1. The dependabot.yml shall `.github/dependabot.yml` に配置されていること。
2. The dependabot.yml shall `cargo` パッケージエコシステムに対して、ルートディレクトリを対象に weekly（月曜日）スケジュールで更新チェックを設定すること。
3. The dependabot.yml shall `github-actions` パッケージエコシステムに対して、weekly（月曜日）スケジュールで更新チェックを設定すること。
4. The dependabot.yml shall Cargo 更新の commit prefix として `chore(deps)` を設定すること。
5. The dependabot.yml shall GitHub Actions 更新の commit prefix として `ci(deps)` を設定すること。
6. The dependabot.yml shall Cargo の minor および patch 更新を `rust-dependencies` グループとしてまとめ、1 PR にバンドルすること。
7. The dependabot.yml shall GitHub Actions の更新を `github-actions` グループとしてまとめ、1 PR にバンドルすること。
8. The dependabot.yml shall Cargo 更新に `dependencies` ラベルを付与し、GitHub Actions 更新に `ci` および `dependencies` ラベルを付与すること。

---

### 要件 6: ファイル内容のプロジェクト整合性

**目的:** プロジェクトメンテナーとして、追加した全ファイルの内容が cupola の実態（技術スタック・ライセンス・ワークフロー）と一致していることを確認したい。そうすることで、誤った情報がコントリビュータに伝わることを防げる。

#### 受け入れ基準

1. The CONTRIBUTING.md shall 使用技術スタックとして Rust（Edition 2024）・devbox・cargo・GitHub Actions を正確に反映すること。
2. The CONTRIBUTING.md shall テストコマンドとして `cargo test` を、静的解析コマンドとして `cargo clippy` を記述すること。
3. The SECURITY.md shall サポートバージョンセクションに cupola の現在の公開状況（最新リリースのみサポート）を正確に反映すること。
4. The dependabot.yml shall `version: 2` を指定し、GitHub の dependabot 設定仕様に準拠すること。
