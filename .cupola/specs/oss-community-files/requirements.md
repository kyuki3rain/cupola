# Requirements Document

## Introduction

cupolaプロジェクトをOSSとして公開・運営するにあたり、国際標準のコミュニティファイル群を整備する。
具体的には、コントリビューションガイド（CONTRIBUTING.md）、セキュリティポリシー（SECURITY.md）、行動規範（CODE_OF_CONDUCT.md）、READMEバッジ、および依存関係自動更新設定（dependabot.yml）を追加する。
これにより、外部コントリビューターの参加障壁を下げ、脆弱性報告の明確な窓口を設け、依存クレートの更新を自動化する。

## Requirements

### Requirement 1: CONTRIBUTING.md の作成

**Objective:** As a 外部コントリビューター, I want 明確な開発参加ガイドが存在すること, so that プロジェクトへの貢献方法を迷わず把握できる

#### Acceptance Criteria

1. The リポジトリ shall `CONTRIBUTING.md` を英語でルートディレクトリに含むこと
2. When コントリビューターが開発環境をセットアップしようとする時, the `CONTRIBUTING.md` shall `devbox shell`、`cargo build`、`cargo test` を使ったセットアップ手順を記載していること
3. The `CONTRIBUTING.md` shall バグ報告・機能提案・PRの作成方法を記載していること
4. The `CONTRIBUTING.md` shall ブランチ命名規則・conventional commits・CIチェックを含むPRプロセスを記載していること
5. The `CONTRIBUTING.md` shall `cargo fmt`・`cargo clippy --all-targets`・テスト要件を含むコーディング規約を記載していること
6. The `CONTRIBUTING.md` shall 前提条件（Rust stable、devbox、gh CLI）を記載していること

### Requirement 2: SECURITY.md の作成

**Objective:** As a セキュリティ研究者またはユーザー, I want 脆弱性の報告先と対応ポリシーが明示されていること, so that 安全に脆弱性を報告できる

#### Acceptance Criteria

1. The リポジトリ shall `SECURITY.md` を英語でルートディレクトリに含むこと
2. The `SECURITY.md` shall GitHub Private Vulnerability Reporting（`https://github.com/kyuki3rain/cupola/security/advisories/new`）を唯一の報告窓口として明記すること
3. The `SECURITY.md` shall 公開Issueでの脆弱性報告を禁止する旨を記載すること
4. The `SECURITY.md` shall 確認タイムライン（48時間以内）および修正タイムライン（90日以内）を記載すること
5. The `SECURITY.md` shall サポート対象バージョン（現時点では最新リリースのみ）を記載すること

### Requirement 3: CODE_OF_CONDUCT.md の作成

**Objective:** As a コミュニティ参加者, I want 行動規範が明示されていること, so that 安心して参加できる環境が保証される

#### Acceptance Criteria

1. The リポジトリ shall `CODE_OF_CONDUCT.md` をルートディレクトリに含むこと
2. The `CODE_OF_CONDUCT.md` shall Contributor Covenant v2.1 の公式テキストを使用すること
3. The `CODE_OF_CONDUCT.md` shall 連絡先（enforcement contact）が適切に設定されていること
4. If 違反が報告された場合, the `CODE_OF_CONDUCT.md` shall 対応プロセスへの参照を提供すること

### Requirement 4: README バッジの追加

**Objective:** As a リポジトリ訪問者, I want CIステータスとライセンス情報をひと目で確認できること, so that プロジェクトの健全性と利用条件を素早く把握できる

#### Acceptance Criteria

1. The `README.md`（英語版）shall ファイル先頭にCIバッジ（`ci.yml` ワークフローへのリンク付き）を含むこと
2. The `README.md`（英語版）shall ファイル先頭にLicenseバッジ（Apache-2.0、`LICENSE` ファイルへのリンク付き）を含むこと
3. The `README.ja.md`（日本語版）shall ファイル先頭に同一のCIバッジおよびLicenseバッジを含むこと
4. When crates.io への公開が完了していない場合, the リポジトリ shall crates.io バッジおよびdocs.rsバッジを追加しないこと

### Requirement 5: .github/dependabot.yml の作成

**Objective:** As a メンテナー, I want 依存関係の更新が自動化されていること, so that セキュリティパッチや新機能を手動確認なく受け取れる

#### Acceptance Criteria

1. The リポジトリ shall `.github/dependabot.yml` を含むこと
2. The `dependabot.yml` shall `cargo` エコシステムを対象とし、毎週月曜日にスケジュールされた更新を設定すること
3. The `dependabot.yml` shall `github-actions` エコシステムを対象とし、毎週月曜日にスケジュールされた更新を設定すること
4. The `dependabot.yml` shall Rustの依存関係について、minor/patchアップデートを1つのPRにまとめるグルーピングを設定すること
5. The `dependabot.yml` shall GitHub Actionsの依存関係について、全更新を1つのPRにまとめるグルーピングを設定すること
6. The `dependabot.yml` shall Cargoの更新PRに `chore(deps)` プレフィックスのコミットメッセージを設定すること
7. The `dependabot.yml` shall GitHub Actionsの更新PRに `ci(deps)` プレフィックスのコミットメッセージを設定すること
8. The `dependabot.yml` shall Cargoの更新PRに `dependencies` ラベルを付与すること
9. The `dependabot.yml` shall GitHub Actionsの更新PRに `ci` および `dependencies` ラベルを付与すること
