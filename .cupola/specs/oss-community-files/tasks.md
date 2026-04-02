# Implementation Plan

## Task Format Template

- [ ] 1. CONTRIBUTING.md を作成する
- [ ] 1.1 (P) コントリビューションガイドの英語ドキュメントを作成する
  - ルートディレクトリに `CONTRIBUTING.md` を英語で作成する
  - セクション構成: Welcome → Prerequisites → Setup → Contributing → PR Process → Coding Standards
  - Prerequisites: Rust stable・devbox・gh CLI を明記する
  - Setup: `devbox shell`・`cargo build`・`cargo test` の手順を記載する
  - Contributing: バグ報告（GitHub Issues）・機能提案・PRの作成方法を記載する
  - PR Process: ブランチ命名規則（例: `feature/xxx`）・conventional commits・CI必須チェックを記載する
  - Coding Standards: `cargo fmt`・`cargo clippy --all-targets`・`#[cfg(test)]` テスト要件を記載する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

- [ ] 2. SECURITY.md を作成する
- [ ] 2.1 (P) セキュリティポリシーの英語ドキュメントを作成する
  - ルートディレクトリに `SECURITY.md` を英語で作成する
  - GitHub Private Vulnerability Reporting URL（`https://github.com/kyuki3rain/cupola/security/advisories/new`）を唯一の報告窓口として明記する
  - 公開Issueでの脆弱性報告を明示的に禁止する旨を記載する
  - 確認タイムライン（48時間以内）・修正タイムライン（90日以内）を記載する
  - サポート対象バージョン（最新リリースのみ）を記載する
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

- [ ] 3. CODE_OF_CONDUCT.md を作成する
- [ ] 3.1 (P) Contributor Covenant v2.1 に基づく行動規範ドキュメントを作成する
  - ルートディレクトリに `CODE_OF_CONDUCT.md` を作成する
  - Contributor Covenant v2.1 公式英語テキストをそのまま使用する（内容を改変しない）
  - `[INSERT CONTACT METHOD]` プレースホルダーを実際の連絡先（メールアドレス等）に置換する
  - プレースホルダーが残っていないことを最終確認する
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [ ] 4. README にバッジを追加する
- [ ] 4.1 (P) 英語版 README.md の先頭にバッジを追加する
  - `README.md` のタイトル行（`# Cupola`）の直下にバッジ行を挿入する
  - CIバッジ: `[![CI](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml/badge.svg)](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml)`
  - Licenseバッジ: `[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/kyuki3rain/cupola/blob/main/LICENSE)`
  - crates.io / docs.rs バッジは追加しない
  - _Requirements: 4.1, 4.2, 4.4_

- [ ] 4.2 (P) 日本語版 README.ja.md の先頭にバッジを追加する
  - `README.ja.md` のタイトル行（`# Cupola`）の直下に英語版と同一のバッジ行を挿入する
  - CIバッジとLicenseバッジの両方を追加する
  - crates.io / docs.rs バッジは追加しない
  - _Requirements: 4.3, 4.4_

- [ ] 5. .github/dependabot.yml を作成する
- [ ] 5.1 (P) Cargo および GitHub Actions の依存関係自動更新設定ファイルを作成する
  - `.github/dependabot.yml` を作成する（`.github/` ディレクトリが存在しない場合は作成する）
  - `cargo` エコシステム: `directory: "/"`, `interval: "weekly"`, `day: "monday"` を設定する
  - `cargo` の `groups` 設定: `patterns: ["*"]`, `update-types: ["minor", "patch"]` でminor/patchをまとめる
  - `cargo` のコミットメッセージプレフィックス: `chore(deps)` を設定する
  - `cargo` のラベル: `dependencies` を設定する
  - `github-actions` エコシステム: `directory: "/"`, `interval: "weekly"`, `day: "monday"` を設定する
  - `github-actions` の `groups` 設定: `patterns: ["*"]` で全更新をまとめる
  - `github-actions` のコミットメッセージプレフィックス: `ci(deps)` を設定する
  - `github-actions` のラベル: `ci`, `dependencies` を設定する
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 5.9_

- [ ] 6. 成果物を検証する
- [ ] 6.1 GitHub Community Profile の認識を確認する
  - `CONTRIBUTING.md`・`SECURITY.md`・`CODE_OF_CONDUCT.md` がGitHub Community Profile（Insights > Community）で認識されることを確認する
  - _Requirements: 1.1, 2.1, 3.1_

- [ ] 6.2 README バッジの表示を確認する
  - `README.md` と `README.ja.md` のバッジがGitHubのレンダリングで正常に表示されることを確認する
  - CIバッジが正しいワークフロー（`ci.yml`）を参照していることを確認する
  - _Requirements: 4.1, 4.2, 4.3_
