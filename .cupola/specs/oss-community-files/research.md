# Research & Design Decisions

---
**Purpose**: OSS標準ファイル追加における調査記録と設計判断の根拠。

---

## Summary

- **Feature**: `oss-community-files`
- **Discovery Scope**: Simple Addition（コードロジックは不要。静的ファイルおよび設定ファイルの追加のみ）
- **Key Findings**:
  - CONTRIBUTING.md・SECURITY.md は英語で記述するのがOSS標準慣行（国際コントリビューター向け）
  - CODE_OF_CONDUCT.md は Contributor Covenant v2.1 が事実上の業界標準
  - dependabot の `groups` 設定により、minor/patch を1PRにまとめてノイズを削減できる

## Research Log

### CONTRIBUTING.md のベストプラクティス

- **Context**: ripgrep・bat・tokio など著名RustプロジェクトのCONTRIBUTING.mdを参考に構成を決定
- **Sources Consulted**: Issue本文（参考リンク）・Rust OSS慣行
- **Findings**:
  - 前提条件（ツール）→ セットアップ → コントリビューション方法 → PRプロセス → コーディング規約 の順が一般的
  - Rustプロジェクトでは `cargo fmt` / `cargo clippy` の明示が必須
  - devbox環境を使用するため、`devbox shell` の手順が必要
- **Implications**: セクション構成を上記順序で統一する

### Contributor Covenant v2.1

- **Context**: CODE_OF_CONDUCT.md の選定
- **Findings**:
  - Contributor Covenant v2.1 は GitHub・Linux Foundation等が採用する業界標準
  - 公式テキスト（英語）をそのまま使用し、enforcement contactを設定する
  - 日本語訳は今回スコープ外（英語テキストで統一）
- **Implications**: 公式テキストを変更せず、連絡先のみカスタマイズ

### dependabot groups 機能

- **Context**: PR乱立を防ぐためのグルーピング設定
- **Findings**:
  - `groups` 機能でminor/patchをまとめることでWeeklyのPR数を大幅削減できる
  - `cargo` と `github-actions` を別エコシステムとして設定することが推奨
  - コミットメッセージプレフィックスにconventional commits形式を使用可能
- **Implications**: Cargo用に `chore(deps)` 、Actions用に `ci(deps)` を設定

### README バッジの配置

- **Context**: CIステータスとライセンス情報の視認性向上
- **Findings**:
  - バッジはREADMEの最上部（タイトル直下）に配置するのが慣行
  - crates.io / docs.rs バッジは公開後に追加するため今回はスキップ
  - 英語版・日本語版の両READMEに同一バッジを追加する
- **Implications**: README.md・README.ja.md の両ファイルを更新対象とする

## Architecture Pattern Evaluation

| オプション | 説明 | 採否 |
|-----------|------|------|
| 静的ファイル追加のみ | コードロジック不要。各ファイルを直接ルートディレクトリまたは `.github/` に配置 | 採用 |
| テンプレート生成スクリプト | 動的生成の仕組みを用意 | 不採用（オーバーエンジニアリング） |

## Design Decisions

### Decision: 言語選択

- **Context**: CONTRIBUTING.md・SECURITY.md の記述言語
- **Alternatives Considered**:
  1. 英語のみ — 国際コントリビューター向け標準慣行
  2. 日本語・英語両方 — 維持コストが増大
- **Selected Approach**: 英語のみ
- **Rationale**: OSS標準慣行に準拠し、維持コストを最小化する
- **Trade-offs**: 日本語話者への敷居がやや高まるが、国際性を優先

### Decision: CODE_OF_CONDUCT の言語

- **Context**: Contributor Covenant テキストの言語選択
- **Selected Approach**: 英語版公式テキストを使用
- **Rationale**: 公式テキストの改変を避け、信頼性を保つ

## Risks & Mitigations

- enforcement contact（CODE_OF_CONDUCT）が未設定のままになるリスク → 設計書でプレースホルダーを明記し、実装タスクで必ず設定を求める
- README バッジのURLが将来変更されるリスク → GitHubのCI URL形式は安定しており低リスク

## References

- [Contributor Covenant v2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/)
- [GitHub Dependabot Groups](https://docs.github.com/en/code-security/dependabot/dependabot-version-updates/configuration-options-for-the-dependabot.yml-file#groups)
- [ripgrep CONTRIBUTING.md](https://github.com/BurntSushi/ripgrep/blob/master/CONTRIBUTING.md)
